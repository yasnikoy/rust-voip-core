use livekit::prelude::*;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use tokio::sync::mpsc;
use parking_lot::Mutex;
use std::borrow::Cow;
use futures::StreamExt;
use nnnoiseless::DenoiseState;
use webrtc_audio_processing::{Processor, InitializationConfig, Config, NoiseSuppression, NoiseSuppressionLevel};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let url = std::env::var("LIVEKIT_URL").unwrap_or_else(|_| "ws://localhost:7880".to_string());
    let token = std::env::var("LIVEKIT_TOKEN").expect("Error: LIVEKIT_TOKEN is missing");

    println!("Connecting to {}...", url);
    let (room, mut event_stream) = Room::connect(&url, &token, RoomOptions::default()).await?;
    println!("Connected to room: {}", room.name());

    let host = cpal::default_host();
    let input_device = host.default_input_device().expect("Error: No microphone found");
    let output_device = host.default_output_device().expect("Error: No speaker found");

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut processor = Processor::new(&InitializationConfig {
        num_capture_channels: 1,
        num_render_channels: 1,
        ..Default::default()
    })?;
    processor.set_config(Config {
        noise_suppression: Some(NoiseSuppression { suppression_level: NoiseSuppressionLevel::VeryHigh }),
        enable_high_pass_filter: true,
        enable_transient_suppressor: true,
        ..Default::default()
    });
    let denoise_state = Arc::new(Mutex::new(DenoiseState::new()));

    let audio_source = NativeAudioSource::new(AudioSourceOptions::default(), 48000, 1, 200);
    room.local_participant().publish_track(
        LocalTrack::Audio(LocalAudioTrack::create_audio_track("main-audio", RtcAudioSource::Native(audio_source.clone()))),
        Default::default()
    ).await?;

    let (mic_tx, mut mic_rx) = mpsc::channel::<Vec<i16>>(1000);
    let source_clone = audio_source.clone();
    tokio::spawn(async move {
        while let Some(data) = mic_rx.recv().await {
            let _ = source_clone.capture_frame(&AudioFrame {
                data: Cow::from(data),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: 480,
            }).await;
        }
    });

    let ds_input = denoise_state.clone();
    let mut accumulator = Vec::with_capacity(480);
    let mic_tx_bridge = mic_tx.clone();

    // Stream değişkenlerini 'guard' olarak adlandıralım ki yaşam sürelerini korudukları belli olsun
    let _input_guard = input_device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            for &sample in data {
                accumulator.push(sample);
                if accumulator.len() >= 480 {
                    // Optimizasyon: accumulator'ı klonlamak yerine üzerinde işlem yapıyoruz
                    processor.process_capture_frame(&mut accumulator).unwrap();
                    
                    let mut output_f32 = [0.0f32; 480];
                    let mut ds = ds_input.lock();
                    ds.process_frame(&mut output_f32, &accumulator);

                    let output_i16: Vec<i16> = output_f32.iter()
                        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                        .collect();

                    let _ = mic_tx_bridge.try_send(output_i16);
                    accumulator.clear();
                }
            }
        },
        |_| {},
        None,
    )?;
    _input_guard.play()?;

    let (out_tx, out_rx) = mpsc::unbounded_channel::<i16>();
    let out_rx_shared = Arc::new(Mutex::new(out_rx));
    let _output_guard = output_device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            let mut rx = out_rx_shared.lock();
            for s in data.iter_mut() {
                *s = rx.try_recv().map(|v| v as f32 / i16::MAX as f32).unwrap_or(0.0);
            }
        },
        |_| {},
        None,
    )?;
    _output_guard.play()?;

    println!("Audio pipeline is active.");

    while let Some(event) = event_stream.recv().await {
        match event {
            RoomEvent::TrackSubscribed { track, .. } => {
                if let RemoteTrack::Audio(audio_track) = track {
                    let otx = out_tx.clone();
                    tokio::spawn(async move {
                        let mut native_stream = NativeAudioStream::new(audio_track.rtc_track(), 48000, 1);
                        while let Some(frame) = native_stream.next().await {
                            for &s in frame.data.iter() { let _ = otx.send(s); }
                        }
                    });
                }
            },
            RoomEvent::Disconnected { reason } => {
                println!("Disconnected: {:?}", reason);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
