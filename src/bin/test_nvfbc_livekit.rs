//! NVFBC → LiveKit Screen Share Test
//! 
//! Tests high-performance screen capture with NVFBC (50+ FPS) 
//! publishing to a real LiveKit room.

use std::sync::Arc;
use std::env;
use dotenv::dotenv;
use livekit::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::RtcVideoSource;
use livekit::options::{TrackPublishOptions, VideoEncoding};

use neandertal_voip_core::nvfbc_capture::{NvfbcScreenShare, is_nvfbc_available};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("🔧 NVFBC → LiveKit Screen Share Test");
    println!("=====================================\n");

    // 1. Check NVFBC availability
    println!("1️⃣  Checking NVFBC availability...");
    if !is_nvfbc_available() {
        eprintln!("❌ NVFBC is not available on this system!");
        eprintln!("   Make sure you have:");
        eprintln!("   - NVIDIA GPU");
        eprintln!("   - NVIDIA drivers installed");
        eprintln!("   - Display connected to NVIDIA GPU");
        return Ok(());
    }
    println!("✅ NVFBC is available!\n");

    // 2. Get LiveKit credentials
    println!("2️⃣  Getting LiveKit credentials...");
    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL must be set in .env");
    let token = env::var("LIVEKIT_TOKEN").expect("LIVEKIT_TOKEN must be set in .env");
    println!("   URL: {}", url);
    println!("   Token: {}...\n", &token[..20.min(token.len())]);

    // 3. Connect to LiveKit
    println!("3️⃣  Connecting to LiveKit...");
    let (room, mut events) = Room::connect(&url, &token, RoomOptions::default()).await?;
    println!("✅ Connected to room: {}\n", room.name());

    // 4. Create video source and track
    println!("4️⃣  Creating video source and track...");
    let screen_source = Arc::new(NativeVideoSource::default());
    let track = LocalVideoTrack::create_video_track(
        "nvfbc_screen_share", 
        RtcVideoSource::Native((*screen_source).clone())
    );
    println!("✅ Video track created\n");

    // 5. Publish track
    println!("5️⃣  Publishing video track...");
    let publication = room.local_participant().publish_track(
        LocalTrack::Video(track.clone()),
        TrackPublishOptions {
            source: TrackSource::Screenshare,
            simulcast: false, // Disable simulcast for testing
            video_encoding: Some(VideoEncoding {
                max_bitrate: 3_000_000, // 3 Mbps
                max_framerate: 60.0,    // 60 FPS target
            }),
            ..Default::default()
        }
    ).await?;
    println!("✅ Track published!");
    println!("   SID: {}", publication.sid());
    println!("   Name: {}\n", publication.name());

    // 6. Start NVFBC capture
    println!("6️⃣  Starting NVFBC capture (60 FPS target)...");
    let nvfbc_capture = NvfbcScreenShare::new(screen_source.clone(), 60)?;
    println!("✅ NVFBC capture started!\n");

    // 7. Display stats periodically
    println!("📊 Streaming... Press Ctrl+C to stop.");
    println!("   Open another browser/app to view the stream.\n");

    // Stats display task
    let track_clone = track.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            
            // Try to get stats
            if let Some(stats) = track_clone.get_stats().await.ok().and_then(|s| s.into_iter().next()) {
                println!("📈 Stats:");
                println!("   - Frames encoded: {:?}", stats);
            }
        }
    });

    // 8. Wait for events
    println!("🔄 Waiting for room events...\n");
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::Disconnected { reason, .. } => {
                println!("🔌 Disconnected: {:?}", reason);
                break;
            }
            RoomEvent::TrackSubscribed { track, publication, participant } => {
                println!("📺 Track subscribed: {} from {}", publication.sid(), participant.identity());
            }
            RoomEvent::TrackUnsubscribed { track, publication, participant } => {
                println!("📺 Track unsubscribed: {} from {}", publication.sid(), participant.identity());
            }
            _ => {}
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up...");
    nvfbc_capture.shutdown();
    println!("✅ Done!");

    Ok(())
}
