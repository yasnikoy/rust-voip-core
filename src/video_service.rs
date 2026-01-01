use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use livekit::webrtc::video_frame::{VideoFrame, I420Buffer};
use livekit::webrtc::video_source::native::NativeVideoSource;
use xcap::Monitor;
use tokio::sync::mpsc;

/// Target FPS for screen capture
const TARGET_FPS: i32 = 60;

/// Alignment requirement for VAAPI/video encoding (must be multiple of 16)
const VAAPI_ALIGNMENT: u32 = 16;

/// Screen share encoding mode
#[derive(Clone, Copy, Debug)]
pub enum EncodingMode {
    /// Software encoding via LiveKit SDK (I420 raw frames)
    Software,
    /// Hardware encoding via VAAPI (H.264 encoded frames)
    /// Note: Requires LiveKit SDK support for encoded frame injection
    #[allow(dead_code)]
    HardwareVaapi,
}

pub struct ScreenShareService {
    _pipeline: gst::Pipeline,
    _appsrc: gst_app::AppSrc,
    _kill_tx: mpsc::Sender<()>,
}

impl ScreenShareService {
    /// Create a new screen share service
    /// 
    /// # Arguments
    /// * `monitor_index` - Index of the monitor to capture
    /// * `source` - LiveKit native video source
    /// * `mode` - Encoding mode (Software or HardwareVaapi)
    /// * `target_resolution` - Target resolution (width, height). Use (0, 0) for native.
    pub fn new(
        monitor_index: usize, 
        source: Arc<NativeVideoSource>,
        mode: EncodingMode,
        target_resolution: (u32, u32),
    ) -> Result<Self> {
        // 1. Set VAAPI environment for Intel Haswell
        std::env::set_var("GST_VAAPI_DRM_DEVICE", "/dev/dri/renderD128");
        std::env::set_var("LIBVA_DRIVER_NAME", "i965");
        
        // 2. Initialize GStreamer
        gst::init()?;

        // 3. Select Monitor
        let monitors = Monitor::all()
            .map_err(|e| anyhow::anyhow!("Failed to enumerate monitors: {}", e))?;
        let monitor = monitors.get(monitor_index)
            .ok_or_else(|| anyhow::anyhow!("Monitor index {} not found (available: {})", monitor_index, monitors.len()))?;
        
        // xcap's width() and height() return Option<u32>
        let native_width = monitor.width().unwrap_or(1920);
        let native_height = monitor.height().unwrap_or(1080);

        // 4. Determine target resolution (aligned to 16 for VAAPI compatibility)
        let (target_width, target_height) = if target_resolution.0 == 0 || target_resolution.1 == 0 {
            // Use native, but align to 16
            (align_to_16(native_width), align_to_16(native_height))
        } else {
            (align_to_16(target_resolution.0), align_to_16(target_resolution.1))
        };

        log::info!("🖥️  Screen Share: {} ({}x{}) → {}x{} [{:?}]", 
            monitor.name().unwrap_or("Unknown".to_string()), 
            native_width, native_height,
            target_width, target_height,
            mode
        );

        // 5. Build appropriate pipeline based on mode
        let pipeline_str = match mode {
            EncodingMode::Software => {
                format!(
                    "appsrc name=screen_src format=time is-live=true do-timestamp=true ! \
                     videoconvert ! \
                     videoscale ! \
                     video/x-raw,format=I420,width={},height={} ! \
                     appsink name=screen_sink emit-signals=true sync=false drop=true max-buffers=1",
                    target_width, target_height
                )
            }
            EncodingMode::HardwareVaapi => {
                // VAAPI hardware encoding pipeline
                // This produces H.264 NAL units - requires LiveKit encoded frame injection support
                format!(
                    "appsrc name=screen_src format=time is-live=true do-timestamp=true ! \
                     videoconvert ! \
                     videoscale ! \
                     video/x-raw,format=I420,width={},height={} ! \
                     vaapih264enc tune=high-compression rate-control=cbr bitrate=3000 keyframe-period=60 ! \
                     h264parse ! \
                     video/x-h264,stream-format=byte-stream,alignment=au ! \
                     appsink name=screen_sink emit-signals=true sync=false drop=true max-buffers=2",
                    target_width, target_height
                )
            }
        };

        let pipeline = gst::parse::launch(&pipeline_str)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Expected gst::Pipeline"))?;

        let appsrc = pipeline
            .by_name("screen_src")
            .ok_or_else(|| anyhow::anyhow!("Missing appsrc"))? 
            .downcast::<gst_app::AppSrc>()
            .map_err(|_| anyhow::anyhow!("Source is not AppSrc"))?;

        let appsink = pipeline
            .by_name("screen_sink")
            .ok_or_else(|| anyhow::anyhow!("Missing appsink"))? 
            .downcast::<gst_app::AppSink>()
            .map_err(|_| anyhow::anyhow!("Sink is not AppSink"))?;

        // 6. Configure input caps (BGRA from xcap)
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRA")
            .field("width", native_width as i32)
            .field("height", native_height as i32)
            .field("framerate", gst::Fraction::new(TARGET_FPS, 1))
            .build();
        appsrc.set_caps(Some(&caps));

        // 7. Configure AppSink callback based on mode
        match mode {
            EncodingMode::Software => {
                Self::setup_software_sink(&appsink, source, target_width, target_height);
            }
            EncodingMode::HardwareVaapi => {
                Self::setup_hardware_sink(&appsink);
            }
        }

        // 8. Spawn capture thread
        let (kill_tx, mut kill_rx) = mpsc::channel(1);
        let monitor_clone = monitors.get(monitor_index).unwrap().clone();
        let appsrc_clone = appsrc.clone();
        let capture_width = native_width;
        let capture_height = native_height;
        
        std::thread::spawn(move || {
            let monitor = monitor_clone;
            let target_interval = Duration::from_micros(16667); // ~60 FPS cap
            
            loop {
                if kill_rx.try_recv().is_ok() {
                    break;
                }

                let start = std::time::Instant::now();
                
                match monitor.capture_image() {
                    Ok(image) => {
                        let expected_size = (capture_width * capture_height * 4) as usize;
                        let raw_bytes = image.into_raw();
                        
                        if raw_bytes.len() == expected_size {
                            let buffer = gst::Buffer::from_slice(raw_bytes);
                            if appsrc_clone.push_buffer(buffer).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Capture error: {}", e);
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                
                // Frame pacing
                let elapsed = start.elapsed();
                if elapsed < target_interval {
                    std::thread::sleep(target_interval - elapsed);
                }
            }
            println!("📹 Capture thread stopped");
        });

        // 9. Start pipeline
        pipeline.set_state(gst::State::Playing)?;
        
        Ok(Self {
            _pipeline: pipeline,
            _appsrc: appsrc,
            _kill_tx: kill_tx,
        })
    }

    /// Setup callback for software encoding mode (I420 -> LiveKit SDK)
    fn setup_software_sink(
        appsink: &gst_app::AppSink, 
        source: Arc<NativeVideoSource>,
        width: u32,
        height: u32
    ) {
        let source_clone = source.clone();
        
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice();

                    // I420 Layout
                    let y_size = (width * height) as usize;
                    let uv_size = (width * height / 4) as usize;
                    
                    if data.len() < y_size + uv_size + uv_size {
                        return Ok(gst::FlowSuccess::Ok);
                    }

                    let mut i420_buf = I420Buffer::new(width, height);
                    let (y_plane, u_plane, v_plane) = i420_buf.data_mut();

                    if y_plane.len() >= y_size && u_plane.len() >= uv_size && v_plane.len() >= uv_size {
                        y_plane[..y_size].copy_from_slice(&data[0..y_size]);
                        u_plane[..uv_size].copy_from_slice(&data[y_size..y_size+uv_size]);
                        v_plane[..uv_size].copy_from_slice(&data[y_size+uv_size..]);
                    }

                    let timestamp_us = buffer.pts()
                        .unwrap_or(gst::ClockTime::ZERO)
                        .nseconds() as i64 / 1000;
                    
                    let mut frame = VideoFrame {
                        buffer: i420_buf,
                        timestamp_us,
                        rotation: livekit::webrtc::video_frame::VideoRotation::VideoRotation0,
                    };
                    
                    source_clone.capture_frame(&mut frame);

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    /// Setup callback for hardware encoding mode (H.264 NAL units)
    /// TODO: Implement when LiveKit SDK supports encoded frame injection
    fn setup_hardware_sink(appsink: &gst_app::AppSink) {
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let h264_data = map.as_slice();
                    
                    // TODO: When LiveKit Rust SDK supports encoded frame injection,
                    // send this H.264 data directly instead of raw frames.
                    // This would bypass the SDK's internal software encoder.
                    //
                    // Example pseudo-code:
                    // source.inject_encoded_frame(h264_data, timestamp, is_keyframe);
                    
                    // For now, just log that we received encoded data
                    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        println!("🎬 VAAPI: Receiving H.264 encoded frames ({} bytes)", h264_data.len());
                        println!("⚠️  Note: Encoded frame injection not yet supported by LiveKit Rust SDK");
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }
    
    /// Convenience constructor with default settings (Software mode, 720p)
    pub fn new_default(monitor_index: usize, source: Arc<NativeVideoSource>) -> Result<Self> {
        Self::new(monitor_index, source, EncodingMode::Software, (1280, 720))
    }
}

/// Align value to nearest multiple of 16 (required for VAAPI/video encoding)
#[must_use]
const fn align_to_16(value: u32) -> u32 {
    (value + (VAAPI_ALIGNMENT - 1)) & !(VAAPI_ALIGNMENT - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to_16() {
        assert_eq!(align_to_16(1920), 1920); // Already aligned
        assert_eq!(align_to_16(1080), 1088); // Needs alignment
        assert_eq!(align_to_16(854), 864);   // 854 -> 864
        assert_eq!(align_to_16(480), 480);   // Already aligned
        assert_eq!(align_to_16(720), 720);   // Already aligned
    }
}
