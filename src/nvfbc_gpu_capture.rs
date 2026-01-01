//! NVFBC capture with GPU-accelerated color conversion
//!
//! Combines NVFBC screen capture with GStreamer OpenGL-based
//! BGRA to I420 conversion for maximum performance.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use nvfbc::{SystemCapturer, BufferFormat};
use nvfbc::system::CaptureMethod;
use livekit::webrtc::video_source::native::NativeVideoSource;

use crate::gpu_color_convert::GpuColorConverter;

/// Check if NVFBC and GPU color conversion are available
pub fn is_gpu_pipeline_available() -> bool {
    // Check NVFBC
    let nvfbc_ok = match nvfbc::SystemCapturer::new() {
        Ok(capturer) => capturer.status().map(|s| s.can_create_now).unwrap_or(false),
        Err(_) => false,
    };
    
    // Check GStreamer OpenGL
    let gst_ok = gstreamer::init().is_ok();
    
    nvfbc_ok && gst_ok
}

/// High-performance NVFBC capture with GPU color conversion
pub struct NvfbcGpuCapture {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl NvfbcGpuCapture {
    /// Create a new NVFBC capture with GPU color conversion
    pub fn new(
        source: Arc<NativeVideoSource>,
        target_fps: u32,
    ) -> Result<Self> {
        // Pre-check NVFBC availability
        let capturer = SystemCapturer::new()
            .map_err(|e| anyhow::anyhow!("NVFBC init failed: {:?}", e))?;
        
        let status = capturer.status()
            .map_err(|e| anyhow::anyhow!("NVFBC status failed: {:?}", e))?;
        
        if !status.can_create_now {
            return Err(anyhow::anyhow!("NVFBC not available"));
        }
        
        let width = status.screen_size.w;
        let height = status.screen_size.h;
        
        drop(capturer); // Release for thread use
        
        println!("🚀 NVFBC+GPU: Screen {}x{} @ {} FPS target", width, height, target_fps);
        
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        
        let handle = std::thread::spawn(move || {
            // Initialize GStreamer in this thread
            if let Err(e) = gstreamer::init() {
                eprintln!("❌ GStreamer init failed: {}", e);
                return;
            }
            
            // Create GPU color converter
            let converter = match GpuColorConverter::new(width, height, source.clone()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ GPU converter init failed: {}", e);
                    return;
                }
            };
            
            // Create NVFBC capturer in this thread
            let mut capturer = match SystemCapturer::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ NVFBC init failed: {:?}", e);
                    return;
                }
            };
            
            // Start capture
            if let Err(e) = capturer.start(BufferFormat::Bgra, target_fps) {
                eprintln!("❌ NVFBC start failed: {:?}", e);
                return;
            }
            
            let mut frame_count = 0u64;
            let start = std::time::Instant::now();
            let frame_timeout = Some(Duration::from_millis(50));
            
            println!("✅ NVFBC+GPU capture loop started");
            
            while running_clone.load(Ordering::Relaxed) {
                match capturer.next_frame(CaptureMethod::Blocking, frame_timeout) {
                    Ok(frame_info) => {
                        // Push BGRA to GPU converter (non-blocking)
                        if let Err(e) = converter.push_bgra_frame(frame_info.buffer) {
                            eprintln!("⚠️  GPU push error: {}", e);
                        }
                        
                        frame_count += 1;
                        if frame_count % 300 == 0 {
                            let fps = frame_count as f64 / start.elapsed().as_secs_f64();
                            println!("📹 NVFBC+GPU: {} frames, {:.1} FPS avg", frame_count, fps);
                        }
                    }
                    Err(e) => {
                        // Timeout is expected, continue
                        if !format!("{:?}", e).contains("Timeout") {
                            eprintln!("⚠️  NVFBC frame error: {:?}", e);
                        }
                    }
                }
            }
            
            let _ = capturer.stop();
            converter.shutdown();
            println!("📹 NVFBC+GPU: Session stopped");
        });
        
        Ok(Self {
            running,
            handle: Some(handle),
        })
    }
    
    /// Shutdown the capture
    pub fn shutdown(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for NvfbcGpuCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
