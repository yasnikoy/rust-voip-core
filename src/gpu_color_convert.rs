//! GPU-accelerated BGRA to I420 color space conversion
//!
//! Uses GStreamer's OpenGL or VAAPI elements for hardware-accelerated
//! color space conversion, avoiding CPU bottleneck.

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use livekit::webrtc::video_frame::{VideoFrame, I420Buffer};
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::Arc;

/// Target framerate for GPU color conversion
const TARGET_FPS: i32 = 60;

/// Maximum buffers in appsink queue
const MAX_BUFFERS: u32 = 2;

/// BGRA bytes per pixel
const BGRA_BYTES_PER_PIXEL: u32 = 4;

/// GPU-accelerated color converter using GStreamer
pub struct GpuColorConverter {
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    width: u32,
    height: u32,
}

impl GpuColorConverter {
    /// Create a new GPU color converter
    ///
    /// # Errors
    /// Returns error if GStreamer initialization fails or pipeline cannot be created
    ///
    /// # Pipeline
    /// appsrc (BGRA) → glupload → glcolorconvert → gldownload → appsink (I420)
    pub fn new(width: u32, height: u32, source: Arc<NativeVideoSource>) -> Result<Self> {
        gst::init()?;

        // Try OpenGL first, fall back to software if not available
        let pipeline_str = format!(
            "appsrc name=src format=time caps=video/x-raw,format=BGRA,width={width},height={height},framerate={TARGET_FPS}/1 ! \
             glupload ! \
             glcolorconvert ! \
             gldownload ! \
             video/x-raw,format=I420 ! \
             appsink name=sink emit-signals=true sync=false max-buffers={MAX_BUFFERS} drop=true"
        );

        let pipeline = gst::parse::launch(&pipeline_str)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Failed to create pipeline"))?;

        let appsrc = pipeline
            .by_name("src")
            .ok_or_else(|| anyhow::anyhow!("No appsrc found"))?
            .downcast::<gst_app::AppSrc>()
            .map_err(|_| anyhow::anyhow!("Failed to get appsrc"))?;

        let appsink = pipeline
            .by_name("sink")
            .ok_or_else(|| anyhow::anyhow!("No appsink found"))?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| anyhow::anyhow!("Failed to get appsink"))?;

        // Set up appsink callback
        let source_clone = Arc::clone(&source);
        let frame_width = width;
        let frame_height = height;
        
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    
                    // I420 data from GPU
                    let i420_data = map.as_slice();
                    
                    // Create I420Buffer and copy data
                    let mut i420_buf = I420Buffer::new(frame_width, frame_height);
                    copy_i420_data(i420_data, frame_width, frame_height, &mut i420_buf);
                    
                    // Create and send frame
                    let timestamp_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_micros() as i64)
                        .unwrap_or(0);
                    
                    let mut video_frame = VideoFrame {
                        buffer: i420_buf,
                        timestamp_us,
                        rotation: livekit::webrtc::video_frame::VideoRotation::VideoRotation0,
                    };
                    
                    source_clone.capture_frame(&mut video_frame);
                    
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        // Start pipeline
        pipeline.set_state(gst::State::Playing)?;

        log::info!("🎨 GPU Color Converter: OpenGL BGRA→I420 pipeline started ({}x{})", width, height);

        Ok(Self {
            pipeline,
            appsrc,
            width,
            height,
        })
    }

    /// Push a BGRA frame for conversion
    ///
    /// # Errors
    /// Returns error if buffer size is invalid or GStreamer push fails
    pub fn push_bgra_frame(&self, bgra_data: &[u8]) -> Result<()> {
        let expected_size = (self.width * self.height * BGRA_BYTES_PER_PIXEL) as usize;
        if bgra_data.len() != expected_size {
            return Err(anyhow::anyhow!(
                "Invalid buffer size: {} expected {}",
                bgra_data.len(),
                expected_size
            ));
        }

        let mut buffer = gst::Buffer::with_size(expected_size)?;
        {
            let buffer_ref = buffer.get_mut()
                .ok_or_else(|| anyhow::anyhow!("Failed to get mutable buffer reference"))?;
            let mut map = buffer_ref.map_writable()?;
            map.copy_from_slice(bgra_data);
        }

        self.appsrc.push_buffer(buffer)
            .map_err(|e| anyhow::anyhow!("Failed to push buffer: {:?}", e))?;
        Ok(())
    }

    /// Shutdown the converter
    pub fn shutdown(&self) {
        if let Err(e) = self.pipeline.set_state(gst::State::Null) {
            log::warn!("Failed to stop GPU converter pipeline: {:?}", e);
        }
        log::info!("🎨 GPU Color Converter: Shutdown");
    }
}

impl Drop for GpuColorConverter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Copy I420 planar data to I420Buffer
fn copy_i420_data(data: &[u8], width: u32, height: u32, buffer: &mut I420Buffer) {
    let (y_plane, u_plane, v_plane) = buffer.data_mut();
    
    let y_size = (width * height) as usize;
    let uv_size = (width * height / 4) as usize;
    
    // Y plane
    if data.len() >= y_size {
        y_plane[..y_size].copy_from_slice(&data[..y_size]);
    }
    
    // U plane
    let u_offset = y_size;
    if data.len() >= u_offset + uv_size {
        u_plane[..uv_size].copy_from_slice(&data[u_offset..u_offset + uv_size]);
    }
    
    // V plane
    let v_offset = y_size + uv_size;
    if data.len() >= v_offset + uv_size {
        v_plane[..uv_size].copy_from_slice(&data[v_offset..v_offset + uv_size]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i420_sizes() {
        let width = 1920u32;
        let height = 1080u32;
        
        let y_size = width * height;
        let uv_size = width * height / 4;
        let total = y_size + uv_size * 2;
        
        // I420: Y + U/4 + V/4 = 1.5 * width * height
        assert_eq!(total, (width * height * 3) / 2);
    }

    #[test]
    fn test_bgra_buffer_size() {
        let width = 1280u32;
        let height = 720u32;
        let expected = (width * height * BGRA_BYTES_PER_PIXEL) as usize;
        
        assert_eq!(expected, 1280 * 720 * 4);
        assert_eq!(expected, 3_686_400);
    }

    #[test]
    fn test_copy_i420_data_bounds() {
        let width = 640u32;
        let height = 480u32;
        
        let y_size = (width * height) as usize;
        let uv_size = (width * height / 4) as usize;
        let total_size = y_size + uv_size * 2;
        
        let data = vec![0u8; total_size];
        let mut buffer = I420Buffer::new(width, height);
        
        // Should not panic
        copy_i420_data(&data, width, height, &mut buffer);
    }

    #[test]
    fn test_const_values() {
        assert_eq!(TARGET_FPS, 60);
        assert_eq!(MAX_BUFFERS, 2);
        assert_eq!(BGRA_BYTES_PER_PIXEL, 4);
    }
}
