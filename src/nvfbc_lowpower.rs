//! Optimized NVFBC capture for low-power systems
//! 
//! Targets 720p @ 30 FPS for systems like Acer Aspire E5-571G
//! with i5-4210U + GeForce 840M

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use nvfbc::{SystemCapturer, BufferFormat};
use nvfbc::system::CaptureMethod;
use livekit::webrtc::video_frame::{VideoFrame, I420Buffer};
use livekit::webrtc::video_source::native::NativeVideoSource;

/// Optimized capture settings for low-power systems
pub struct LowPowerSettings {
    pub target_width: u32,
    pub target_height: u32,
    pub target_fps: u32,
}

impl Default for LowPowerSettings {
    fn default() -> Self {
        Self {
            target_width: 1280,
            target_height: 720,
            target_fps: 30,
        }
    }
}

/// Frame timeout for low-power capture (milliseconds)
const FRAME_TIMEOUT_MS: u64 = 100;

/// Frames between FPS reports
const FPS_REPORT_INTERVAL: u64 = 150;

/// Optimized NVFBC capture for low-power laptops
pub struct NvfbcLowPowerCapture {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl NvfbcLowPowerCapture {
    pub fn new(
        source: Arc<NativeVideoSource>,
        settings: LowPowerSettings,
    ) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        
        let handle = std::thread::spawn(move || {
            if let Err(e) = Self::capture_loop(source, settings, running_clone) {
                log::error!("Low-power capture loop failed: {:?}", e);
            }
        });
        
        Ok(Self {
            running,
            handle: Some(handle),
        })
    }
    
    /// Main capture loop - separated for better error handling
    fn capture_loop(
        source: Arc<NativeVideoSource>,
        settings: LowPowerSettings,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        // Create capturer in thread
        let mut capturer = SystemCapturer::new()
            .map_err(|e| anyhow::anyhow!("NVFBC init failed: {:?}", e))?;
        
        let status = capturer.status()
            .map_err(|e| anyhow::anyhow!("NVFBC status failed: {:?}", e))?;
        
        let src_width = status.screen_size.w;
        let src_height = status.screen_size.h;
        
        log::info!("🖥️  Source: {}x{}", src_width, src_height);
        log::info!("🎯 Target: {}x{} @ {} FPS", 
            settings.target_width, settings.target_height, settings.target_fps);
        
        // Start capture at lower FPS
        capturer.start(BufferFormat::Bgra, settings.target_fps)
            .map_err(|e| anyhow::anyhow!("NVFBC start failed: {:?}", e))?;
        
        let mut frame_count = 0u64;
        let start = std::time::Instant::now();
        let frame_timeout = Some(Duration::from_millis(FRAME_TIMEOUT_MS));
        
        // Pre-allocate buffers for target resolution
        let target_width = settings.target_width;
        let target_height = settings.target_height;
        
        log::info!("✅ Low-power capture started");
        
        while running.load(Ordering::Relaxed) {
            match capturer.next_frame(CaptureMethod::Blocking, frame_timeout) {
                Ok(frame_info) => {
                    // Downscale + convert
                    let mut i420_buf = I420Buffer::new(target_width, target_height);
                    
                    // Fast downscale + convert
                    bgra_to_i420_scaled(
                        frame_info.buffer,
                        frame_info.width, frame_info.height,
                        target_width, target_height,
                        &mut i420_buf
                    );
                    
                    // Send to LiveKit
                    let timestamp_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_micros() as i64)
                        .unwrap_or(0);
                    
                    let mut video_frame = VideoFrame {
                        buffer: i420_buf,
                        timestamp_us,
                        rotation: livekit::webrtc::video_frame::VideoRotation::VideoRotation0,
                    };
                    
                    source.capture_frame(&mut video_frame);
                    
                    frame_count += 1;
                    if frame_count % FPS_REPORT_INTERVAL == 0 {
                        let fps = frame_count as f64 / start.elapsed().as_secs_f64();
                        log::info!("📹 Low-power: {} frames, {:.1} FPS", frame_count, fps);
                    }
                }
                Err(_) => {
                    // Timeout, continue
                }
            }
        }
        
        capturer.stop()
            .map_err(|e| anyhow::anyhow!("NVFBC stop failed: {:?}", e))?;
        log::info!("📹 Low-power capture stopped");
        
        Ok(())
    }
    
    pub fn shutdown(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for NvfbcLowPowerCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

// Color conversion constants (BT.601)
const YUV_Y_R_COEF: i32 = 66;
const YUV_Y_G_COEF: i32 = 129;
const YUV_Y_B_COEF: i32 = 25;
const YUV_U_R_COEF: i32 = -38;
const YUV_U_G_COEF: i32 = -74;
const YUV_U_B_COEF: i32 = 112;
const YUV_V_R_COEF: i32 = 112;
const YUV_V_G_COEF: i32 = -94;
const YUV_V_B_COEF: i32 = -18;
const YUV_ROUNDING: i32 = 128;
const YUV_SHIFT: i32 = 8;
const YUV_Y_OFFSET: i32 = 16;
const YUV_UV_OFFSET: i32 = 128;

/// BGRA pixel indices
const BGRA_B: usize = 0;
const BGRA_G: usize = 1;
const BGRA_R: usize = 2;
const BYTES_PER_PIXEL: usize = 4;

/// Fast BGRA to I420 with nearest-neighbor downscaling
///
/// Uses BT.601 color space conversion coefficients.
/// Performs nearest-neighbor downscaling for performance on low-power systems.
fn bgra_to_i420_scaled(
    bgra: &[u8],
    src_width: u32, src_height: u32,
    dst_width: u32, dst_height: u32,
    i420: &mut I420Buffer,
) {
    let (y_plane, u_plane, v_plane) = i420.data_mut();
    
    let src_w = src_width as usize;
    let src_h = src_height as usize;
    let dst_w = dst_width as usize;
    let dst_h = dst_height as usize;
    
    // Scale factors (16-bit fixed point)
    let x_ratio = (src_w << 16) / dst_w;
    let y_ratio = (src_h << 16) / dst_h;
    
    // Process Y plane (full resolution)
    for dst_y in 0..dst_h {
        let src_y = (dst_y * y_ratio) >> 16;
        for dst_x in 0..dst_w {
            let src_x = (dst_x * x_ratio) >> 16;
            let bgra_idx = (src_y * src_w + src_x) * BYTES_PER_PIXEL;
            
            if bgra_idx + BGRA_R >= bgra.len() { 
                continue; 
            }
            
            let blue = i32::from(bgra[bgra_idx + BGRA_B]);
            let green = i32::from(bgra[bgra_idx + BGRA_G]);
            let red = i32::from(bgra[bgra_idx + BGRA_R]);
            
            let y_value = ((YUV_Y_R_COEF * red + YUV_Y_G_COEF * green + YUV_Y_B_COEF * blue + YUV_ROUNDING) >> YUV_SHIFT) + YUV_Y_OFFSET;
            y_plane[dst_y * dst_w + dst_x] = y_value.clamp(0, 255) as u8;
        }
    }
    
    // Process U/V planes (half resolution - chroma subsampling)
    for dst_y in 0..dst_h/2 {
        for dst_x in 0..dst_w/2 {
            let src_x = ((dst_x * 2) * x_ratio) >> 16;
            let src_y = ((dst_y * 2) * y_ratio) >> 16;
            let bgra_idx = (src_y * src_w + src_x) * BYTES_PER_PIXEL;
            
            if bgra_idx + BGRA_R >= bgra.len() { 
                continue; 
            }
            
            let blue = i32::from(bgra[bgra_idx + BGRA_B]);
            let green = i32::from(bgra[bgra_idx + BGRA_G]);
            let red = i32::from(bgra[bgra_idx + BGRA_R]);
            
            let u_value = ((YUV_U_R_COEF * red + YUV_U_G_COEF * green + YUV_U_B_COEF * blue + YUV_ROUNDING) >> YUV_SHIFT) + YUV_UV_OFFSET;
            let v_value = ((YUV_V_R_COEF * red + YUV_V_G_COEF * green + YUV_V_B_COEF * blue + YUV_ROUNDING) >> YUV_SHIFT) + YUV_UV_OFFSET;
            
            let uv_idx = dst_y * (dst_w / 2) + dst_x;
            if uv_idx < u_plane.len() {
                u_plane[uv_idx] = u_value.clamp(0, 255) as u8;
                v_plane[uv_idx] = v_value.clamp(0, 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = LowPowerSettings::default();
        assert_eq!(settings.target_width, 1280);
        assert_eq!(settings.target_height, 720);
        assert_eq!(settings.target_fps, 30);
    }
}
