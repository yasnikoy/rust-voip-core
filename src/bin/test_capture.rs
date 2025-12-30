use std::time::{Instant, Duration};
use xcap::Monitor;

fn main() -> anyhow::Result<()> {
    println!("🔍 Xcap Screen Capture Performance Test");
    println!("=======================================");

    let monitors = Monitor::all()?;

    if monitors.is_empty() {
        eprintln!("❌ No monitors found!");
        return Ok(());
    }

    println!("Detected Monitors:");
    for (i, monitor) in monitors.iter().enumerate() {
        println!("{}. {} ({}x{} @ {},{})", 
            i, 
            monitor.name().unwrap_or("Unknown".into()), 
            monitor.width().unwrap_or(0), 
            monitor.height().unwrap_or(0), 
            monitor.x().unwrap_or(0), 
            monitor.y().unwrap_or(0)
        );
    }

    let monitor = monitors.first().unwrap();
    println!("\n🚀 Starting capture test on: {}", monitor.name().unwrap_or("Unknown".into()));
    println!("   Duration: 10 seconds...");

    let start_time = Instant::now();
    let mut frame_count = 0;
    let mut last_log = Instant::now();
    let mut min_latency = Duration::from_secs(1);
    let mut max_latency = Duration::from_secs(0);

    // Save one frame for verification
    let mut saved_first_frame = false;

    while start_time.elapsed() < Duration::from_secs(10) {
        let frame_start = Instant::now();
        
        match monitor.capture_image() {
            Ok(image) => {
                let latency = frame_start.elapsed();
                frame_count += 1;

                if latency < min_latency { min_latency = latency; }
                if latency > max_latency { max_latency = latency; }

                if !saved_first_frame {
                    let filename = format!("capture_test_{}.png", monitor.id().unwrap_or(0));
                    // xcap returns image::RgbaImage
                    image.save(&filename)?;
                    println!("📸 Saved first frame to '{}'", filename);
                    saved_first_frame = true;
                }
            }
            Err(e) => {
                eprintln!("❌ Capture error: {}", e);
            }
        }

        if last_log.elapsed() > Duration::from_secs(1) {
            print!(".");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            last_log = Instant::now();
        }
    }

    let total_duration = start_time.elapsed();
    let fps = frame_count as f64 / total_duration.as_secs_f64();

    println!("\n\n📊 Test Results:");
    println!("----------------");
    println!("Total Frames: {}", frame_count);
    println!("Total Time:   {:.2}s", total_duration.as_secs_f64());
    println!("Average FPS:  {:.2}", fps);
    println!("Latency (Capture only):");
    println!("  Min: {:.2}ms", min_latency.as_secs_f64() * 1000.0);
    println!("  Max: {:.2}ms", max_latency.as_secs_f64() * 1000.0);
    println!("  Avg: {:.2}ms", (total_duration.as_secs_f64() * 1000.0) / frame_count as f64);

    if fps < 30.0 {
        println!("\n⚠️  Performance Warning: FPS < 30. This might be due to CPU load or capture method inefficiencies.");
    } else {
        println!("\n✅ Performance is good for real-time streaming.");
    }

    Ok(())
}
