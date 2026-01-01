//! Low-power NVFBC → LiveKit test
//! 
//! Optimized for Acer Aspire E5-571G (i5-4210U + 840M)
//! Target: 720p @ 30 FPS with minimal CPU usage

use std::sync::Arc;
use std::env;
use dotenv::dotenv;
use livekit::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::RtcVideoSource;
use livekit::options::{TrackPublishOptions, VideoEncoding};

use neandertal_voip_core::nvfbc_lowpower::{NvfbcLowPowerCapture, LowPowerSettings};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("🔋 Low-Power NVFBC → LiveKit Test");
    println!("==================================");
    println!("Optimized for: Acer Aspire E5-571G");
    println!("Target: 720p @ 30 FPS\n");

    // Get credentials
    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL must be set");
    let token = env::var("LIVEKIT_TOKEN").expect("LIVEKIT_TOKEN must be set");

    // Connect to LiveKit
    println!("1️⃣  Connecting to LiveKit...");
    let (room, mut events) = Room::connect(&url, &token, RoomOptions::default()).await?;
    println!("✅ Connected to: {}\n", room.name());

    // Create video source
    println!("2️⃣  Creating video track...");
    let source = Arc::new(NativeVideoSource::default());
    let track = LocalVideoTrack::create_video_track(
        "lowpower_screen", 
        RtcVideoSource::Native((*source).clone())
    );

    // Publish with conservative settings
    let publication = room.local_participant().publish_track(
        LocalTrack::Video(track.clone()),
        TrackPublishOptions {
            source: TrackSource::Screenshare,
            simulcast: false, // Save CPU
            video_encoding: Some(VideoEncoding {
                max_bitrate: 1_500_000, // 1.5 Mbps for 720p
                max_framerate: 30.0,
            }),
            ..Default::default()
        }
    ).await?;
    println!("✅ Published: {}\n", publication.sid());

    // Start low-power capture
    println!("3️⃣  Starting low-power capture...");
    let settings = LowPowerSettings {
        target_width: 1280,
        target_height: 720,
        target_fps: 30,
    };
    let capture = NvfbcLowPowerCapture::new(source.clone(), settings)?;
    println!("✅ Capture started!\n");

    println!("📊 Streaming at 720p @ 30 FPS...");
    println!("   Expected CPU usage: < 30%");
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

    capture.shutdown();
    println!("✅ Done!");

    Ok(())
}
