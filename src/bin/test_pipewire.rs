//! PipeWire Screen Capture Test
//! 
//! Tests the high-performance PipeWire Portal screen capture backend.

use std::sync::Arc;
use neandertal_voip_core::pipewire_capture::PipeWireScreenShare;
use livekit::webrtc::video_source::native::NativeVideoSource;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    println!("🔧 PipeWire Screen Capture Test");
    println!("================================");
    println!("This will show the XDG Desktop Portal screen selection dialog.");
    println!("Select a monitor to capture.\n");
    
    // Create a dummy video source for testing
    let source = Arc::new(NativeVideoSource::default());
    
    // Start PipeWire screen share
    println!("Starting PipeWire capture...");
    let screen_share = PipeWireScreenShare::new(source).await?;
    
    println!("\n✅ PipeWire capture started!");
    println!("Capturing frames for 10 seconds...\n");
    
    // Run for 10 seconds
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    
    // Shutdown
    println!("\n🛑 Shutting down...");
    screen_share.shutdown().await?;
    
    println!("✅ Test complete!");
    
    Ok(())
}
