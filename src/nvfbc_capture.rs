//! NVFBC-based screen capture service
//!
//! High-performance screen capture using NVIDIA Frame Buffer Capture API.
//! Bypasses X11 completely and reads directly from GPU framebuffer.
//!
//! ## Performance
//! - Captures at 50-60+ FPS on X11
//! - Near zero CPU overhead
//! - Works on GeForce, Quadro, Tesla GPUs

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use nvfbc::{SystemCapturer, BufferFormat};
use nvfbc::system::CaptureMethod;
use livekit::webrtc::video_frame::{VideoFrame, I420Buffer};
use livekit::webrtc::video_source::native::NativeVideoSource;

/// Frame timeout for capture (milliseconds)
const FRAME_TIMEOUT_MS: u64 = 50;

/// Frames between FPS reports
const FPS_REPORT_INTERVAL: u64 = 300;

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

/// BGRA pixel component indices
const BGRA_B: usize = 0;
const BGRA_G: usize = 1;
const BGRA_R: usize = 2;
const BYTES_PER_PIXEL: usize = 4;

/// Check if NVFBC is available on this system
#[must_use]
pub fn is_nvfbc_available() -> bool {
    SystemCapturer::new()
        .ok()
        .and_then(|capturer| capturer.status().ok())
        .map_or(false, |status| status.can_create_now)
}

/// NVFBC-based screen share service
pub struct NvfbcScreenShare {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl NvfbcScreenShare {
    /// Create a new NVFBC screen share session
    ///
    /// # Errors
    /// Returns an error if NVFBC is not available or initialization fails.
    pub fn new(
        source: Arc<NativeVideoSource>,
        target_fps: u32,
    ) -> Result<Self> {
        // Check availability first (on main thread)
        if !is_nvfbc_available() {
            return Err(anyhow::anyhow!("NVFBC is not available on this system"));
        }
        
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        
        // Create capturer inside the thread since it's not Send
        let handle = std::thread::spawn(move || {
            if let Err(e) = Self::capture_loop(source, target_fps, running_clone) {
                log::error!("NVFBC capture loop failed: {:?}", e);
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
        target_fps: u32,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        // Create capturer in this thread
        let mut capturer = SystemCapturer::new()
            .map_err(|e| anyhow::anyhow!("NVFBC init failed in thread: {:?}", e))?;
        
        let status = capturer.status()
            .map_err(|e| anyhow::anyhow!("NVFBC status failed: {:?}", e))?;
        
        let width = status.screen_size.w;
        let height = status.screen_size.h;
        log::info!("🖥️  NVFBC: Screen {}x{} @ {} FPS target", width, height, target_fps);
        
        // Start capture
        capturer.start(BufferFormat::Bgra, target_fps)
            .map_err(|e| anyhow::anyhow!("NVFBC start failed: {:?}", e))?;
        
        let mut frame_count = 0u64;
        let start = std::time::Instant::now();
        let frame_timeout = Some(Duration::from_millis(FRAME_TIMEOUT_MS));
        
        log::info!("✅ NVFBC capture loop started");
        
        while running.load(Ordering::Relaxed) {
            match capturer.next_frame(CaptureMethod::Blocking, frame_timeout) {
                Ok(frame_info) => {
                    // Convert BGRA to I420 for LiveKit
                    let mut i420_buf = I420Buffer::new(frame_info.width, frame_info.height);
                    bgra_to_i420(frame_info.buffer, frame_info.width, frame_info.height, &mut i420_buf);
                    
                    // Create VideoFrame
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
                    if frame_count.is_multiple_of(FPS_REPORT_INTERVAL) {
                        let fps = frame_count as f64 / start.elapsed().as_secs_f64();
                        log::info!("📹 NVFBC: {} frames, {:.1} FPS avg", frame_count, fps);
                    }
                }
                Err(e) => {
                    // Timeout is OK, just continue
                    if !format!("{e:?}").contains("Timeout") {
                        log::warn!("⚠️  NVFBC frame error: {:?}", e);
                    }
                }
            }
        }
        
        capturer.stop()
            .map_err(|e| anyhow::anyhow!("NVFBC stop failed: {:?}", e))?;
        log::info!("📹 NVFBC: Session stopped");
        
        Ok(())
    }
    
    /// Shutdown the screen share
    pub fn shutdown(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for NvfbcScreenShare {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // Note: We can't join the thread in Drop as it might panic
    }
}

/// Convert BGRA to I420 (BT.601 color space)
///
/// Processes 2x2 pixel blocks for chroma subsampling.
fn bgra_to_i420(bgra: &[u8], width: u32, height: u32, i420: &mut I420Buffer) {
    let (y_plane, u_plane, v_plane) = i420.data_mut();
    
    let width_usize = width as usize;
    let height_usize = height as usize;
    
    // Process 2x2 blocks for chroma subsampling
    for block_y in 0..height_usize/2 {
        for block_x in 0..width_usize/2 {
            let mut red_sum = 0i32;
            let mut green_sum = 0i32;
            let mut blue_sum = 0i32;
            
            // Process 4 pixels in 2x2 block
            for dy in 0..2 {
                for dx in 0..2 {
                    let pixel_x = block_x * 2 + dx;
                    let pixel_y = block_y * 2 + dy;
                    let bgra_idx = (pixel_y * width_usize + pixel_x) * BYTES_PER_PIXEL;
                    
                    if bgra_idx + BGRA_R >= bgra.len() {
                        continue;
                    }
                    
                    let blue = i32::from(bgra[bgra_idx + BGRA_B]);
                    let green = i32::from(bgra[bgra_idx + BGRA_G]);
                    let red = i32::from(bgra[bgra_idx + BGRA_R]);
                    
                    // Calculate Y for this pixel
                    let y_value = ((YUV_Y_R_COEF * red + YUV_Y_G_COEF * green + YUV_Y_B_COEF * blue + YUV_ROUNDING) >> YUV_SHIFT) + YUV_Y_OFFSET;
                    y_plane[pixel_y * width_usize + pixel_x] = y_value.clamp(0, 255) as u8;
                    
                    red_sum += red;
                    green_sum += green;
                    blue_sum += blue;
                }
            }
            
            // Average for U and V (chroma subsampling)
            let red_avg = red_sum / 4;
            let green_avg = green_sum / 4;
            let blue_avg = blue_sum / 4;
            
            let u_value = ((YUV_U_R_COEF * red_avg + YUV_U_G_COEF * green_avg + YUV_U_B_COEF * blue_avg + YUV_ROUNDING) >> YUV_SHIFT) + YUV_UV_OFFSET;
            let v_value = ((YUV_V_R_COEF * red_avg + YUV_V_G_COEF * green_avg + YUV_V_B_COEF * blue_avg + YUV_ROUNDING) >> YUV_SHIFT) + YUV_UV_OFFSET;
            
            let uv_idx = block_y * (width_usize / 2) + block_x;
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
    fn test_nvfbc_availability_check() {
        // Just ensure function doesn't panic
        let _available = is_nvfbc_available();
    }
}
