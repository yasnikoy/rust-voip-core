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
        let running_clone = running.clone();
        
        let handle = std::thread::spawn(move || {
            // Create capturer in thread
            let mut capturer = match SystemCapturer::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ NVFBC init failed: {:?}", e);
                    return;
                }
            };
            
            let status = match capturer.status() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ NVFBC status failed: {:?}", e);
                    return;
                }
            };
            
            let src_width = status.screen_size.w;
            let src_height = status.screen_size.h;
            
            println!("🖥️  Source: {}x{}", src_width, src_height);
            println!("🎯 Target: {}x{} @ {} FPS", 
                settings.target_width, settings.target_height, settings.target_fps);
            
            // Start capture at lower FPS
            if let Err(e) = capturer.start(BufferFormat::Bgra, settings.target_fps) {
                eprintln!("❌ NVFBC start failed: {:?}", e);
                return;
            }
            
            let mut frame_count = 0u64;
            let start = std::time::Instant::now();
            let frame_timeout = Some(Duration::from_millis(100));
            
            // Pre-allocate buffers for target resolution
            let tw = settings.target_width;
            let th = settings.target_height;
            
            println!("✅ Low-power capture started");
            
            while running_clone.load(Ordering::Relaxed) {
                match capturer.next_frame(CaptureMethod::Blocking, frame_timeout) {
                    Ok(frame_info) => {
                        // Downscale + convert
                        let mut i420_buf = I420Buffer::new(tw, th);
                        
                        // Fast downscale + convert
                        bgra_to_i420_scaled(
                            frame_info.buffer,
                            frame_info.width, frame_info.height,
                            tw, th,
                            &mut i420_buf
                        );
                        
                        // Send to LiveKit
                        let timestamp_us = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as i64;
                        
                        let mut video_frame = VideoFrame {
                            buffer: i420_buf,
                            timestamp_us,
                            rotation: livekit::webrtc::video_frame::VideoRotation::VideoRotation0,
                        };
                        
                        source.capture_frame(&mut video_frame);
                        
                        frame_count += 1;
                        if frame_count % 150 == 0 {
                            let fps = frame_count as f64 / start.elapsed().as_secs_f64();
                            println!("📹 Low-power: {} frames, {:.1} FPS", frame_count, fps);
                        }
                    }
                    Err(_) => {
                        // Timeout, continue
                    }
                }
            }
            
            let _ = capturer.stop();
            println!("📹 Low-power capture stopped");
        });
        
        Ok(Self {
            running,
            handle: Some(handle),
        })
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

/// Fast BGRA to I420 with nearest-neighbor downscaling
fn bgra_to_i420_scaled(
    bgra: &[u8],
    src_w: u32, src_h: u32,
    dst_w: u32, dst_h: u32,
    i420: &mut I420Buffer,
) {
    let (y_plane, u_plane, v_plane) = i420.data_mut();
    
    let sw = src_w as usize;
    let sh = src_h as usize;
    let dw = dst_w as usize;
    let dh = dst_h as usize;
    
    // Scale factors
    let x_ratio = (sw << 16) / dw;
    let y_ratio = (sh << 16) / dh;
    
    // Process Y plane (full resolution)
    for j in 0..dh {
        let src_y = (j * y_ratio) >> 16;
        for i in 0..dw {
            let src_x = (i * x_ratio) >> 16;
            let idx = (src_y * sw + src_x) * 4;
            
            if idx + 3 >= bgra.len() { continue; }
            
            let b = bgra[idx] as i32;
            let g = bgra[idx + 1] as i32;
            let r = bgra[idx + 2] as i32;
            
            let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
            y_plane[j * dw + i] = y.clamp(0, 255) as u8;
        }
    }
    
    // Process U/V planes (half resolution)
    for j in 0..dh/2 {
        for i in 0..dw/2 {
            let src_x = ((i * 2) * x_ratio) >> 16;
            let src_y = ((j * 2) * y_ratio) >> 16;
            let idx = (src_y * sw + src_x) * 4;
            
            if idx + 3 >= bgra.len() { continue; }
            
            let b = bgra[idx] as i32;
            let g = bgra[idx + 1] as i32;
            let r = bgra[idx + 2] as i32;
            
            let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
            let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
            
            let uv_idx = j * (dw / 2) + i;
            if uv_idx < u_plane.len() {
                u_plane[uv_idx] = u.clamp(0, 255) as u8;
                v_plane[uv_idx] = v.clamp(0, 255) as u8;
            }
        }
    }
}
