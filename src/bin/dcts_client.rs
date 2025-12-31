use neandertal_voip_core::audio_service;
use neandertal_voip_core::video_service;
use livekit::prelude::*;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::RtcVideoSource;
use livekit::options::{TrackPublishOptions, VideoEncoding};
use std::sync::Arc;
use parking_lot::Mutex;
use std::env;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL must be set");
    let token = env::var("LIVEKIT_TOKEN").expect("LIVEKIT_TOKEN must be set");

    println!("🔌 Connecting to LiveKit: {}", url);

    let (room, mut events) = Room::connect(&url, &token, RoomOptions::default()).await?;
    println!("✅ Connected to room: {}", room.name());

    // --- 1. START AUDIO (Loopback / Local Processing) ---
    // Note: This audio service currently plays to local speakers (Loopback).
    // In a real app, we would feed the 'processed' audio to LiveKit's LocalAudioTrack.
    // For this demo, we just start it to verify it runs alongside screen share without crashing.
    
    let audio_settings = Arc::new(Mutex::new(audio_service::AudioSettings::default()));
    let audio_state = Arc::new(audio_service::GlobalAudioState { 
        is_transmitting: std::sync::atomic::AtomicBool::new(true)
    });

    audio_service::start_input_listener(audio_state.clone(), audio_settings.clone());
    
    // Start Audio Session on Default Device
    let _audio_session = audio_service::AudioSession::create("default", audio_state, audio_service::AudioSettings::default());
    
    if let Err(e) = &_audio_session {
        eprintln!("⚠️ Audio Init Failed: {}", e);
    } else {
        println!("🎤 Audio Service Started (Local Loopback)");
    }

    // --- 2. START SCREEN SHARE ---
    println!("🖥️  Starting Screen Share...");
    
    // Create Native Source
    let screen_source = NativeVideoSource::default();
    // Create Track
    let track = LocalVideoTrack::create_video_track("screen_share", RtcVideoSource::Native(screen_source.clone()));
    
    // Publish Track
    let publication = room.local_participant().publish_track(LocalTrack::Video(track), TrackPublishOptions {
        source: TrackSource::Screenshare,
        simulcast: false, // Disable simulcast to save CPU
        video_encoding: Some(VideoEncoding {
            max_bitrate: 2_500_000,
            max_framerate: 60.0,
        }),
        ..Default::default()
    }).await?;
    
    println!("📡 Screen Track Published: {}", publication.sid());

    // Start Capture & Encode Pipeline
    // We use monitor index 0 (Primary), Software encoding, 720p target
    let _video_service = video_service::ScreenShareService::new(
        0, 
        Arc::new(screen_source),
        video_service::EncodingMode::Software,
        (1280, 720) // 720p - good balance of quality and performance
    );

    match _video_service {
        Ok(_) => println!("🎥 Screen Share Service Running (GStreamer -> LiveKit)"),
        Err(e) => eprintln!("❌ Screen Share Failed: {}", e),
    }

    // Keep connection alive
    // Handle events (optional)
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::Disconnected { reason, .. } => {
                println!("Disconnected: {:?}", reason);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
