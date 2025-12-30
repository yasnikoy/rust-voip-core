use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use anyhow::Result;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use livekit::webrtc::video_frame::{VideoFrame, VideoBuffer, I420Buffer};
use livekit::webrtc::video_source::{RtcVideoSource, native::NativeVideoSource};
use livekit::webrtc::prelude::*;
use xcap::Monitor;
use tokio::sync::mpsc;

pub struct ScreenShareService {
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    _kill_tx: mpsc::Sender<()>
}

impl ScreenShareService {
    pub fn new(monitor_index: usize, source: Arc<NativeVideoSource>) -> Result<Self> {
        // 1. Initialize GStreamer
        gst::init()?;

        // 2. Select Monitor
        let monitors = Monitor::all()?;
        let monitor = monitors.get(monitor_index)
            .ok_or_else(|| anyhow::anyhow!("Monitor index {} not found", monitor_index))?;
        
        let width = monitor.width().unwrap_or(1920);
        let height = monitor.height().unwrap_or(1080);
        let monitor_id = monitor.id().unwrap_or(0); // Assuming ID is stable for now

        println!("🖥️  Screen Share: Selected Monitor {} ({}x{})", monitor.name().unwrap_or("Unknown".into()), width, height);

        // 3. Setup GStreamer Pipeline
        // appsrc (BGRA) -> videoconvert -> videoscale -> video/x-raw,format=I420,width=848,height=480 -> appsink
        // We scale to 480p aligned (848x480) to maximize FPS (target 60) and avoid stride issues.
        let target_width = 848;
        let target_height = 480;
        
        let pipeline_str = format!(
            "appsrc name=screen_src format=time is-live=true do-timestamp=true ! \
             videoconvert ! \
             videoscale ! \
             video/x-raw,format=I420,width={},height={} ! \
             appsink name=screen_sink emit-signals=true sync=false drop=true max-buffers=1",
             target_width, target_height
        );

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

        // 4. Configure Caps
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRA") // xcap usually gives BGRA compatible
            .field("width", width as i32)
            .field("height", height as i32)
            .field("framerate", gst::Fraction::new(60, 1)) // Input capped at 60
            .build();
        appsrc.set_caps(Some(&caps));

        // 5. Configure AppSink Callback
        let source_clone = source.clone();
        
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice();

                    // I420 Layout: Y plane (w*h) + U plane (w/2 * h/2) + V plane (w/2 * h/2)
                    let width = target_width;
                    let height = target_height;
                    
                    let y_size = (width * height) as usize;
                    let u_size = (width * height / 4) as usize;
                    // let v_size = u_size;
                    
                    if data.len() < y_size + u_size + u_size {
                         return Ok(gst::FlowSuccess::Ok);
                    }

                    let (stride_y, stride_u, stride_v) = (width as i32, (width/2) as i32, (width/2) as i32);
                    
                    let mut i420_buf = I420Buffer::new(width as u32, height as u32);
                    
                    // Copy planes
                    // data_mut() returns (stride_y, stride_u, stride_v, data_y, data_u, data_v) in some libs
                    // OR returns (y_plane, u_plane, v_plane) as slices.
                    // Based on compiler error: `(&mut [u8], &mut [u8], &mut [u8])`
                    
                    let (y_plane, u_plane, v_plane) = i420_buf.data_mut();

                    // Check sizes just in case, though new() should allocate enough.
                    // We assume input data is tightly packed I420 from GStreamer.
                    // GStreamer I420 is usually contiguous.
                    
                    if y_plane.len() >= y_size && u_plane.len() >= u_size && v_plane.len() >= u_size {
                         y_plane[..y_size].copy_from_slice(&data[0..y_size]);
                         u_plane[..u_size].copy_from_slice(&data[y_size..y_size+u_size]);
                         v_plane[..u_size].copy_from_slice(&data[y_size+u_size..]);
                    } else {
                        // Stride mismatch or padding? GStreamer might add padding?
                        // For now just warn/skip or try best effort copy line by line if strides differ.
                        // Assuming tight packing for MVP.
                    }

                    // Create Frame
                    // duration is in nanoseconds
                    let duration_us = buffer.pts().unwrap_or(gst::ClockTime::ZERO).nseconds() as i64 / 1000;
                    
                    // Attempt direct struct initialization assuming fields are public or checking error message for field names
                    let mut frame = VideoFrame {
                        buffer: i420_buf,
                        timestamp_us: duration_us,
                        rotation: livekit::webrtc::video_frame::VideoRotation::VideoRotation0,
                    };
                    
                    // Send to LiveKit
                    source_clone.capture_frame(&mut frame);

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        // 6. Spawn Capture Thread
        let (kill_tx, mut kill_rx) = mpsc::channel(1);
        let monitor_clone = monitors.get(monitor_index).unwrap().clone(); // Clone monitor? xcap Monitor isn't cloneable easily?
        // xcap Monitor is not Clone in 0.8? We have to re-fetch or keep index.
        // We will re-fetch inside thread or move it if possible. xcap Monitor is just a struct, likely moveable.
        
        // We need a thread that captures from xcap and pushes to appsrc.
        let appsrc_clone = appsrc.clone();
        
        std::thread::spawn(move || {
            // Re-fetch monitor to be safe or use the moved one if xcap Monitor is Send.
            // Assuming monitor is Send.
            let monitor = monitor_clone; // Move in
            
            loop {
                if kill_rx.try_recv().is_ok() {
                    break;
                }

                let start = std::time::Instant::now();
                
                match monitor.capture_image() {
                    Ok(image) => {
                        let size = (width * height * 4) as usize;
                        let raw_bytes = image.into_raw();
                        
                        if raw_bytes.len() == size {
                            let buffer = gst::Buffer::from_slice(raw_bytes);
                            if let Err(_) = appsrc_clone.push_buffer(buffer) {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Capture error: {}", e);
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                
                // Cap at ~60 FPS
                let elapsed = start.elapsed();
                if elapsed < Duration::from_millis(16) {
                    std::thread::sleep(Duration::from_millis(16) - elapsed);
                }
            }
        });
        
        // 6. Handle AppSink (Converted Frame -> LiveKit)
        // Whenever appsink gets an I420 frame, we convert it to LiveKit VideoFrame and push.
        
        // Note: Connecting signals in Rust GStreamer is verbose.
        // appsink.set_callbacks(...) is better.
        
        // For now, let's just return the service object. The thread above handles capture -> pipeline.
        // We need the SINK side to feed LiveKit.
        
        // ... I will implement the sink callback in the next step to keep file size manageable.
        
        pipeline.set_state(gst::State::Playing)?;
        
        Ok(Self {
            pipeline,
            appsrc,
            _kill_tx: kill_tx,
        })
    }
}
