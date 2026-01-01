//! NVFBC Screen Capture Test
//! 
//! Tests NVIDIA Frame Buffer Capture for high-performance X11 screen capture.
//! This bypasses X11 completely and reads directly from GPU framebuffer.

use nvfbc::{SystemCapturer, BufferFormat};
use nvfbc::system::CaptureMethod;
use std::time::{Instant, Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 NVFBC Screen Capture Test");
    println!("============================");
    println!("This tests NVIDIA GPU framebuffer capture (60+ FPS potential)\n");
    
    // Try to create the capturer
    println!("Creating NVFBC capturer...");
    let mut capturer = match SystemCapturer::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Failed to create NVFBC capturer: {}", e);
            eprintln!("\nPossible reasons:");
            eprintln!("  - NVIDIA driver doesn't support NVFBC");
            eprintln!("  - Not running on NVIDIA GPU display");
            eprintln!("  - Driver patch needed for GeForce cards");
            return Ok(());
        }
    };
    
    // Check status
    let status = capturer.status()?;
    println!("📊 NVFBC Status:");
    println!("  - Can create now: {}", status.can_create_now);
    println!("  - Current display mode: {}x{}", 
        status.screen_size.w, status.screen_size.h);
    
    if !status.can_create_now {
        eprintln!("❌ Can't create NVFBC session right now");
        return Ok(());
    }
    
    // Start capture at 60 FPS
    println!("\n🚀 Starting capture at 60 FPS...");
    capturer.start(BufferFormat::Bgra, 60)?;
    
    // Capture 300 frames and measure performance
    let frame_count = 300;
    let start = Instant::now();
    
    for i in 0..frame_count {
        let frame_info = capturer.next_frame(CaptureMethod::Blocking, Some(Duration::from_millis(100)))?;
        
        if i == 0 {
            println!("📐 Frame size: {}x{}", frame_info.width, frame_info.height);
            println!("📦 Buffer size: {} bytes", frame_info.buffer.len());
        }
        
        if (i + 1) % 100 == 0 {
            let elapsed = start.elapsed();
            let fps = (i + 1) as f64 / elapsed.as_secs_f64();
            println!("📹 Frame {}: {:.1} FPS", i + 1, fps);
        }
    }
    
    let elapsed = start.elapsed();
    let avg_fps = frame_count as f64 / elapsed.as_secs_f64();
    
    println!("\n📊 Results:");
    println!("  - Frames captured: {}", frame_count);
    println!("  - Time elapsed: {:.2}s", elapsed.as_secs_f64());
    println!("  - Average FPS: {:.1}", avg_fps);
    
    capturer.stop()?;
    println!("\n✅ Test complete!");
    
    Ok(())
}
