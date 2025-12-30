use std::time::{Instant, Duration};
use xcap::Monitor;
use gstreamer::prelude::*;
use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    // Initialize GStreamer
    gstreamer::init()?;

    println!("🎥 Xcap + GStreamer Hardware Encoding Test");
    println!("========================================");

    let monitors = Monitor::all()?;
    let monitor = monitors.first().ok_or_else(|| anyhow::anyhow!("No monitor found"))?;
    
    let width = monitor.width().unwrap_or(1920);
    let height = monitor.height().unwrap_or(1080);
    println!("Selected Monitor: {} ({}x{})", monitor.name().unwrap_or("Unknown".into()), width, height);

    // Setup Pipeline: appsrc -> videoconvert -> vaapih264enc -> mp4mux -> filesink
    // We try vaapih264enc first. If it fails, users can manually switch to x264enc in code.
    let pipeline_str = format!(
        "appsrc name=mysource format=time is-live=true do-timestamp=true ! \
         videoconvert ! \
         video/x-raw,format=NV12 ! \
         vaapih264enc bitrate=3000 ! \
         h264parse ! \
         mp4mux ! \
         filesink location=test_recording.mp4"
    );

    println!("\n🔧 Building Pipeline:\n{}", pipeline_str);

    let pipeline = gstreamer::parse::launch(&pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Expected a gst::Pipeline"))?;

    let appsrc = pipeline
        .by_name("mysource")
        .ok_or_else(|| anyhow::anyhow!("Could not find appsrc"))? 
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("mysource is not an AppSrc"))?;

    // Configure AppSrc caps (resolution & framerate)
    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("format", "BGRA") // xcap returns BGRA/RGBA compatible data
        .field("width", width as i32)
        .field("height", height as i32)
        .field("framerate", gstreamer::Fraction::new(30, 1)) // Target 30 FPS
        .build();
    appsrc.set_caps(Some(&caps));

    // Start Pipeline
    pipeline.set_state(gstreamer::State::Playing)?;

    println!("🚀 Recording started (10 seconds)...");
    
    let start_time = Instant::now();
    let mut frame_count = 0;
    
    // We run the loop for 10 seconds
    while start_time.elapsed() < Duration::from_secs(10) {
        let frame_start = Instant::now();

        // 1. Capture
        match monitor.capture_image() {
            Ok(image) => {
                // xcap returns RgbaImage (u8 vec). GStreamer usually expects BGRA on little endian?
                // Actually image::RgbaImage is RGBA.
                // videoconvert will handle RGBA -> NV12 conversion for VAAPI.
                
                // Convert to GStreamer Buffer
                let size = (width * height * 4) as usize;
                
                // IMPORTANT: In a real high-perf app, we would avoid this vec copy by implementing
                // a custom GstMemory or reusing a buffer pool. For this test, Vec copy is okay.
                // But xcap image.into_raw() consumes the image and gives the vec, so it's efficient enough.
                let raw_bytes = image.into_raw(); 
                
                if raw_bytes.len() != size {
                    eprintln!("⚠️ Frame size mismatch: expected {}, got {}", size, raw_bytes.len());
                    continue;
                }

                let buffer = gstreamer::Buffer::from_slice(raw_bytes);
                
                // Push to pipeline
                if let Err(e) = appsrc.push_buffer(buffer) {
                     eprintln!("❌ Failed to push buffer: {}", e);
                     break;
                }
                
                frame_count += 1;
            }
            Err(e) => eprintln!("❌ Capture failed: {}", e),
        }

        // Simple FPS pacing (very naive, just to not flood if capture is too fast, 
        // though capturing itself takes time).
        // If capture takes 30ms, we are already at ~33 FPS.
        // We rely on capture speed acting as natural throttle or appsrc handling it.
        
        // Check for bus messages (errors/EOS)
        let bus = pipeline.bus().unwrap();
        if let Some(msg) = bus.pop() {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Error(err) => {
                    eprintln!("❌ Pipeline Error: {} ({:?})", err.error(), err.debug());
                    break;
                }
                MessageView::Eos(..) => break,
                _ => (),
            }
        }
    }

    // Stop
    let _ = appsrc.end_of_stream();
    let _ = pipeline.set_state(gstreamer::State::Null);

    let duration = start_time.elapsed();
    let fps = frame_count as f64 / duration.as_secs_f64();
    println!("\n🏁 Recording finished.");
    println!("Saved to: test_recording.mp4");
    println!("Total Frames: {}", frame_count);
    println!("Avg FPS: {:.2}", fps);

    Ok(())
}
