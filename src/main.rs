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

// --- SETTINGS INFRASTRUCTURE ---

#[derive(Clone, Copy)]
struct AudioSettings {
    input_gain: f32,
    output_gain: f32,
    gate_threshold: f32,
    gate_hold_frames: u32,
    ns_level: NoiseSuppressionLevel,
    enable_rnnoise: bool,
    enable_hpf: bool,
    enable_ts: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            input_gain: 1.0,
            output_gain: 1.0,
            gate_threshold: 0.0, // Varsayılan olarak kapalı (0.0)
            gate_hold_frames: 20,
            ns_level: NoiseSuppressionLevel::VeryHigh,
            enable_rnnoise: true,
            enable_hpf: true,
            enable_ts: true,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    // 1. Settings & Shared State
    let settings = Arc::new(Mutex::new(AudioSettings::default()));

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

    // 2. Initial Processor Setup
    let mut processor = Processor::new(&InitializationConfig {
        num_capture_channels: 1,
        num_render_channels: 1,
        ..Default::default()
    })?;
    
    // İşlemciyi ilk ayarlarla başlat
    {
        let s = settings.lock();
        processor.set_config(Config {
            noise_suppression: Some(NoiseSuppression { suppression_level: s.ns_level }),
            enable_high_pass_filter: s.enable_hpf,
            enable_transient_suppressor: s.enable_ts,
            ..Default::default()
        });
    }

    let denoise_state = Arc::new(Mutex::new(DenoiseState::new()));

    // 3. LiveKit Transport
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

    // 4. Input Capture Loop with Dynamic Settings
    let settings_input = settings.clone();
    let ds_input = denoise_state.clone();
    let mut accumulator = Vec::with_capacity(480);
    let mic_tx_bridge = mic_tx.clone();
    
    // Gate States
    let mut is_gate_open = false;
    let mut current_hold = 0;

    let _input_guard = input_device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            let s = settings_input.lock(); // Her callback'te ayarları oku
            
            for &sample in data {
                accumulator.push(sample * s.input_gain); // Gain uygulanmış veri
                
                if accumulator.len() >= 480 {
                    // WebRTC Processing
                    processor.process_capture_frame(&mut accumulator).unwrap();
                    
                    // Voice Activity / Gate
                    let max_amplitude = accumulator.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
                    if max_amplitude > s.gate_threshold {
                        is_gate_open = true;
                        current_hold = s.gate_hold_frames;
                    } else {
                        if current_hold > 0 { current_hold -= 1; } else { is_gate_open = false; }
                    }

                    let mut output_f32 = [0.0f32; 480];
                    if is_gate_open {
                        if s.enable_rnnoise {
                            let mut ds = ds_input.lock();
                            ds.process_frame(&mut output_f32, &accumulator);
                        } else {
                            output_f32.copy_from_slice(&accumulator);
                        }
                    }

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

    // 5. Output Playback Loop with Dynamic Settings
    let settings_output = settings.clone();
    let (out_tx, out_rx) = mpsc::unbounded_channel::<i16>();
    let out_rx_shared = Arc::new(Mutex::new(out_rx));
    let _output_guard = output_device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            let s = settings_output.lock();
            let mut rx = out_rx_shared.lock();
            for out_sample in data.iter_mut() {
                *out_sample = rx.try_recv()
                    .map(|v| (v as f32 / i16::MAX as f32) * s.output_gain) // Çıkış Gain
                    .unwrap_or(0.0);
            }
        },
        |_| {},
        None,
    )?;
    _output_guard.play()?;

    println!("Audio pipeline active with dynamic settings support.");

    // Demo: Ayarların dinamik olarak değiştiğini simüle etmek için 5 saniye sonra Gain'i artırabiliriz
    // let s_clone = settings.clone();
    // tokio::spawn(async move {
    //     tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    //     println!("--- Dynamic update: Increasing gain to 2.0 ---");
    //     s_clone.lock().input_gain = 2.0;
    // });

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