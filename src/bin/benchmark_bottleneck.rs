//! Benchmark: Find the actual bottleneck
//!
//! Tests different stages to identify where FPS drops:
//! 1. NVFBC only
//! 2. NVFBC + CPU BGRA→I420
//! 3. NVFBC + GPU BGRA→I420
//! 4. NVFBC + CPU BGRA→I420 + LiveKit capture_frame

use std::time::{Instant, Duration};
use nvfbc::{SystemCapturer, BufferFormat};
use nvfbc::system::CaptureMethod;
use livekit::webrtc::video_frame::{VideoFrame, I420Buffer};
use livekit::webrtc::video_source::native::NativeVideoSource;

const FRAME_COUNT: usize = 300;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn main() -> anyhow::Result<()> {
    println!("🔬 Bottleneck Analysis");
    println!("======================\n");

    // Test 1: Pure NVFBC
    println!("Test 1: Pure NVFBC capture (no conversion)...");
    let fps1 = test_nvfbc_only()?;
    println!("   Result: {:.1} FPS\n", fps1);

    // Test 2: NVFBC + CPU BGRA→I420
    println!("Test 2: NVFBC + CPU BGRA→I420 conversion...");
    let fps2 = test_nvfbc_cpu_convert()?;
    println!("   Result: {:.1} FPS\n", fps2);

    // Test 3: NVFBC + CPU BGRA→I420 + LiveKit capture_frame
    println!("Test 3: NVFBC + CPU + LiveKit capture_frame (full pipeline)...");
    let fps3 = test_nvfbc_cpu_livekit()?;
    println!("   Result: {:.1} FPS\n", fps3);

    // Summary
    println!("📊 Summary:");
    println!("   NVFBC only:           {:.1} FPS", fps1);
    println!("   + CPU conversion:     {:.1} FPS (overhead: {:.1}%)", fps2, (1.0 - fps2/fps1) * 100.0);
    println!("   + LiveKit capture:    {:.1} FPS (overhead: {:.1}%)", fps3, (1.0 - fps3/fps2) * 100.0);
    println!("\n🎯 Bottleneck: {}", if fps3/fps2 < 0.8 { "LiveKit capture_frame()" } else if fps2/fps1 < 0.8 { "CPU conversion" } else { "NVFBC capture" });

    Ok(())
}

fn test_nvfbc_only() -> anyhow::Result<f64> {
    let mut capturer = SystemCapturer::new()?;
    capturer.start(BufferFormat::Bgra, 60)?;
    
    let start = Instant::now();
    for _ in 0..FRAME_COUNT {
        let _ = capturer.next_frame(CaptureMethod::Blocking, Some(Duration::from_millis(50)))?;
    }
    let elapsed = start.elapsed();
    capturer.stop()?;
    
    Ok(FRAME_COUNT as f64 / elapsed.as_secs_f64())
}

fn test_nvfbc_cpu_convert() -> anyhow::Result<f64> {
    let mut capturer = SystemCapturer::new()?;
    capturer.start(BufferFormat::Bgra, 60)?;
    
    let start = Instant::now();
    for _ in 0..FRAME_COUNT {
        let frame = capturer.next_frame(CaptureMethod::Blocking, Some(Duration::from_millis(50)))?;
        
        // CPU conversion
        let mut i420_buf = I420Buffer::new(frame.width, frame.height);
        bgra_to_i420(frame.buffer, frame.width, frame.height, &mut i420_buf);
    }
    let elapsed = start.elapsed();
    capturer.stop()?;
    
    Ok(FRAME_COUNT as f64 / elapsed.as_secs_f64())
}

fn test_nvfbc_cpu_livekit() -> anyhow::Result<f64> {
    let source = NativeVideoSource::default();
    
    let mut capturer = SystemCapturer::new()?;
    capturer.start(BufferFormat::Bgra, 60)?;
    
    let start = Instant::now();
    for _ in 0..FRAME_COUNT {
        let frame = capturer.next_frame(CaptureMethod::Blocking, Some(Duration::from_millis(50)))?;
        
        // CPU conversion
        let mut i420_buf = I420Buffer::new(frame.width, frame.height);
        bgra_to_i420(frame.buffer, frame.width, frame.height, &mut i420_buf);
        
        // LiveKit capture
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
    }
    let elapsed = start.elapsed();
    capturer.stop()?;
    
    Ok(FRAME_COUNT as f64 / elapsed.as_secs_f64())
}

/// CPU BGRA to I420 conversion
fn bgra_to_i420(bgra: &[u8], width: u32, height: u32, i420: &mut I420Buffer) {
    let (y_plane, u_plane, v_plane) = i420.data_mut();
    
    let w = width as usize;
    let h = height as usize;
    
    for j in 0..h/2 {
        for i in 0..w/2 {
            let mut r_sum = 0i32;
            let mut g_sum = 0i32;
            let mut b_sum = 0i32;
            
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = i * 2 + dx;
                    let py = j * 2 + dy;
                    let idx = (py * w + px) * 4;
                    
                    if idx + 3 >= bgra.len() { continue; }
                    
                    let b = bgra[idx] as i32;
                    let g = bgra[idx + 1] as i32;
                    let r = bgra[idx + 2] as i32;
                    
                    let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
                    y_plane[py * w + px] = y.clamp(0, 255) as u8;
                    
                    r_sum += r;
                    g_sum += g;
                    b_sum += b;
                }
            }
            
            let r_avg = r_sum / 4;
            let g_avg = g_sum / 4;
            let b_avg = b_sum / 4;
            
            let u = ((-38 * r_avg - 74 * g_avg + 112 * b_avg + 128) >> 8) + 128;
            let v = ((112 * r_avg - 94 * g_avg - 18 * b_avg + 128) >> 8) + 128;
            
            let uv_idx = j * (w / 2) + i;
            if uv_idx < u_plane.len() {
                u_plane[uv_idx] = u.clamp(0, 255) as u8;
                v_plane[uv_idx] = v.clamp(0, 255) as u8;
            }
        }
    }
}
