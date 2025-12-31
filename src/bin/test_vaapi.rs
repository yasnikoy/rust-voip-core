/// VAAPI Hardware Encoding Test
/// 
/// This test verifies that the VAAPI hardware encoder is working
/// and measures the encoding performance.

use std::time::{Instant, Duration};
use std::sync::Arc;
use parking_lot::Mutex;
use gstreamer as gst;
use gst::prelude::*;
use xcap::Monitor;

fn main() -> anyhow::Result<()> {
    // Set VAAPI environment for Intel Haswell
    std::env::set_var("GST_VAAPI_DRM_DEVICE", "/dev/dri/renderD128");
    std::env::set_var("LIBVA_DRIVER_NAME", "i965");
    
    println!("🔧 VAAPI Hardware Encoding Test");
    println!("================================");
    
    // Initialize GStreamer
    gst::init()?;
    
    // Get monitor info
    let monitors = Monitor::all()?;
    let monitor = monitors.first().ok_or_else(|| anyhow::anyhow!("No monitor"))?;
    let width = monitor.width().unwrap_or(1920);
    let height = monitor.height().unwrap_or(1080);
    
    println!("📺 Monitor: {} ({}x{})", monitor.name().unwrap_or("Unknown".into()), width, height);
    
    // Test 1: Software Pipeline (baseline)
    println!("\n📊 Test 1: Software Scale/Convert (Baseline)");
    test_pipeline(&format!(
        "appsrc name=src format=time is-live=true ! \
         videoconvert ! videoscale ! video/x-raw,format=I420,width=1280,height=720 ! \
         fakesink sync=false"
    ), monitor, width, height, 300)?;
    
    // Test 2: VAAPI Encoding Pipeline
    println!("\n📊 Test 2: VAAPI H.264 Encoding");
    test_pipeline(&format!(
        "appsrc name=src format=time is-live=true ! \
         videoconvert ! videoscale ! video/x-raw,format=I420,width=1280,height=720 ! \
         vaapih264enc tune=high-compression rate-control=cbr bitrate=3000 ! \
         fakesink sync=false"
    ), monitor, width, height, 300)?;
    
    println!("\n✅ Test complete!");
    
    Ok(())
}

fn test_pipeline(
    pipeline_str: &str, 
    monitor: &Monitor, 
    width: u32, 
    height: u32, 
    frames: u32
) -> anyhow::Result<()> {
    let pipeline = gst::parse::launch(pipeline_str)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Not a pipeline"))?;
    
    let appsrc = pipeline.by_name("src")
        .ok_or_else(|| anyhow::anyhow!("No appsrc"))?
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("Not appsrc"))?;
    
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "BGRA")
        .field("width", width as i32)
        .field("height", height as i32)
        .field("framerate", gst::Fraction::new(60, 1))
        .build();
    appsrc.set_caps(Some(&caps));
    
    pipeline.set_state(gst::State::Playing)?;
    
    // Wait for pipeline to start
    std::thread::sleep(Duration::from_millis(500));
    
    let frames_sent = Arc::new(Mutex::new(0u32));
    let frames_sent_clone = frames_sent.clone();
    
    let start = Instant::now();
    
    for _ in 0..frames {
        match monitor.capture_image() {
            Ok(image) => {
                let expected_size = (width * height * 4) as usize;
                let raw = image.into_raw();
                
                if raw.len() == expected_size {
                    let buffer = gst::Buffer::from_slice(raw);
                    if appsrc.push_buffer(buffer).is_ok() {
                        *frames_sent.lock() += 1;
                    }
                }
            }
            Err(e) => eprintln!("Capture error: {}", e),
        }
    }
    
    // Signal EOS and wait
    let _ = appsrc.end_of_stream();
    std::thread::sleep(Duration::from_millis(500));
    
    let elapsed = start.elapsed();
    let sent = *frames_sent_clone.lock();
    let fps = sent as f64 / elapsed.as_secs_f64();
    
    println!("   Frames: {} / {}", sent, frames);
    println!("   Time:   {:.2}s", elapsed.as_secs_f64());
    println!("   FPS:    {:.2}", fps);
    
    pipeline.set_state(gst::State::Null)?;
    
    Ok(())
}
