use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use parking_lot::Mutex;
use ringbuf::{HeapRb, Rb};
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
            gate_threshold: 0.015,
            gate_hold_frames: 20,
            ns_level: NoiseSuppressionLevel::VeryHigh,
            enable_rnnoise: true,
            enable_hpf: true,
            enable_ts: true,
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let settings = Arc::new(Mutex::new(AudioSettings::default()));

    println!("🚀 Starting Local Audio Loopback...");

    let host = cpal::default_host();
    let input_device = host.default_input_device().expect("No input device");
    let output_device = host.default_output_device().expect("No output device");

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Default,
    };

    // --- WebRTC Processor Setup (FIXED) ---
    let mut processor = Processor::new(&InitializationConfig {
        num_capture_channels: 1,
        num_render_channels: 1,
        ..Default::default()
    })?;

    // Ayarları işlemciye net bir şekilde gönderelim
    {
        let s = settings.lock();
        processor.set_config(Config {
            noise_suppression: Some(NoiseSuppression {
                suppression_level: s.ns_level,
            }),
            enable_high_pass_filter: s.enable_hpf,
            enable_transient_suppressor: s.enable_ts,
            ..Default::default()
        });
    }

    let rb = HeapRb::<f32>::new(48000);
    let (mut prod, mut cons) = rb.split();

    let denoise_state = Arc::new(Mutex::new(DenoiseState::new()));
    let settings_input = settings.clone();
    let ds_input = denoise_state.clone();
    let mut accumulator = Vec::with_capacity(480);
    
    let mut is_gate_open = false;
    let mut current_hold = 0;

    let _input_guard = input_device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            let s = settings_input.lock();
            
            for &sample in data {
                accumulator.push(sample * s.input_gain);
                
                if accumulator.len() >= 480 {
                    // 1. WebRTC Pre-processing (NS + HPF + TS)
                    processor.process_capture_frame(&mut accumulator).unwrap();
                    
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
                            // 2. RNNoise Post-processing
                            let mut ds = ds_input.lock();
                            ds.process_frame(&mut output_f32, &accumulator);
                        } else {
                            output_f32.copy_from_slice(&accumulator);
                        }
                    }

                    for &s_out in &output_f32 {
                        let _ = prod.push(s_out);
                    }
                    accumulator.clear();
                }
            }
        },
        |_| {},
        None,
    )?;

    let settings_output = settings.clone();
    let _output_guard = output_device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            let s = settings_output.lock();
            for out_sample in data.iter_mut() {
                *out_sample = cons.pop().map(|v| v * s.output_gain).unwrap_or(0.0);
            }
        },
        |_| {},
        None,
    )?;

    _input_guard.play()?;
    _output_guard.play()?;

    std::thread::park();
    Ok(())
}