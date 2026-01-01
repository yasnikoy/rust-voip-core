//! NVFBC + GPU Color Convert → LiveKit Test
//! 
//! Tests high-performance screen capture with:
//! - NVFBC for capture (50+ FPS potential)
//! - GStreamer OpenGL for BGRA→I420 conversion (GPU accelerated)
//! - LiveKit for streaming

use std::sync::Arc;
use std::env;
use dotenv::dotenv;
use livekit::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::RtcVideoSource;
use livekit::options::{TrackPublishOptions, VideoEncoding};

use neandertal_voip_core::nvfbc_gpu_capture::NvfbcGpuCapture;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("🚀 NVFBC + GPU Color Convert → LiveKit Test");
    println!("=============================================\n");

    // 1. Get LiveKit credentials
    println!("1️⃣  Getting LiveKit credentials...");
    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL must be set in .env");
    let token = env::var("LIVEKIT_TOKEN").expect("LIVEKIT_TOKEN must be set in .env");
    println!("   URL: {}\n", url);

    // 2. Connect to LiveKit
    println!("2️⃣  Connecting to LiveKit...");
    let (room, mut events) = Room::connect(&url, &token, RoomOptions::default()).await?;
    println!("✅ Connected to room: {}\n", room.name());

    // 3. Create video source and track
    println!("3️⃣  Creating video source...");
    let screen_source = Arc::new(NativeVideoSource::default());
    let track = LocalVideoTrack::create_video_track(
        "nvfbc_gpu_screen", 
        RtcVideoSource::Native((*screen_source).clone())
    );
    println!("✅ Video track created\n");

    // 4. Publish track
    println!("4️⃣  Publishing video track...");
    let publication = room.local_participant().publish_track(
        LocalTrack::Video(track.clone()),
        TrackPublishOptions {
            source: TrackSource::Screenshare,
            simulcast: false,
            video_encoding: Some(VideoEncoding {
                max_bitrate: 4_000_000, // 4 Mbps for high quality
                max_framerate: 60.0,
            }),
            ..Default::default()
        }
    ).await?;
    println!("✅ Track published: {}\n", publication.sid());

    // 5. Start NVFBC + GPU capture
    println!("5️⃣  Starting NVFBC + GPU capture pipeline...");
    let capture = NvfbcGpuCapture::new(screen_source.clone(), 60)?;
    println!("✅ GPU-accelerated capture started!\n");

    println!("📊 Streaming with GPU acceleration...");
    println!("   Expected: 50+ FPS (vs ~28 FPS with CPU conversion)");
    println!("   Press Ctrl+C to stop.\n");

    // Wait for events
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::Disconnected { reason, .. } => {
                println!("🔌 Disconnected: {:?}", reason);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up...");
    capture.shutdown();
    println!("✅ Done!");

    Ok(())
}
