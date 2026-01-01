use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use parking_lot::Mutex;
use ringbuf::HeapRb;
use ringbuf::traits::{Split, Producer, Consumer, Observer};
use nnnoiseless::DenoiseState;
use webrtc_audio_processing::{Processor, InitializationConfig, Config, NoiseSuppression, NoiseSuppressionLevel};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use std::time::Duration;
use rdev::{listen, Event, EventType, Key};

// --- MODELS ---


#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub display_name: String,
}

pub struct GlobalAudioState {
    pub is_transmitting: AtomicBool,
}

#[derive(Clone)]
pub struct AudioSettings {
    pub input_device_id: String,
    pub ptt_key: Key,
    pub ptt_enabled: bool,
    pub aec_enabled: bool,
    pub agc_enabled: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { 
            input_device_id: "default".to_string(),
            ptt_key: Key::ControlLeft,
            ptt_enabled: false,
            aec_enabled: true,
            agc_enabled: true,
        }
    }
}

pub struct AudioSession {
    // Keep streams alive
    _streams: (cpal::Stream, cpal::Stream),
}

// --- DEVICE DISCOVERY ---

pub fn get_professional_device_list() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let mut list = Vec::new();
    let mut seen_friendly = std::collections::HashSet::new();

    list.push(AudioDeviceInfo { 
        id: "default".to_string(), 
        display_name: "Sistem Varsayılanı (Pulse/Pipewire)".to_string() 
    });
    seen_friendly.insert("Sistem Varsayılanı (Pulse/Pipewire)".to_string());

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            #[allow(deprecated)]
            let name_res = device.name();
            
            if let Ok(id) = name_res {
                let l_id = id.to_lowercase();
                
                if l_id.starts_with("hw:") || 
                   l_id.contains("dmix") || 
                   l_id.contains("dsnoop") || 
                   l_id.contains("surround") || 
                   l_id.contains("front") || 
                   l_id.contains("rear") || 
                   l_id.contains("center") || 
                   l_id.contains("side") || 
                   l_id.contains("iec958") || 
                   l_id.contains("hdmi") || 
                   l_id.contains("null") ||
                   id == "default" {
                    continue;
                }

                let is_reliable = l_id.contains("sysdefault") || l_id.contains("plughw");
                if !is_reliable {
                    continue; 
                }

                let clean_name = if let Some(card_part) = id.split("CARD=").nth(1) {
                    let raw_name = card_part.split(',').next().unwrap_or(card_part);
                    if l_id.contains("sysdefault") {
                        format!("{} (System Default)", raw_name)
                    } else {
                        format!("{} (Direct/Plug)", raw_name)
                    }
                } else {
                    id.clone()
                };

                if !seen_friendly.contains(&clean_name) {
                    list.push(AudioDeviceInfo { id, display_name: clean_name.clone() });
                    seen_friendly.insert(clean_name);
                }
            }
        }
    }
    
    list.sort_by(|a, b| {
        if a.id == "default" { std::cmp::Ordering::Less }
        else if b.id == "default" { std::cmp::Ordering::Greater }
        else { a.display_name.cmp(&b.display_name) }
    });
    
    list
}

// --- SESSION LOGIC ---

impl AudioSession {
    pub fn create(in_id: &str, state: Arc<GlobalAudioState>, settings: AudioSettings) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let in_device = if in_id == "default" {
            host.default_input_device().ok_or_else(|| anyhow::anyhow!("No mic found"))?
        } else {
            let mut devices = host.input_devices()?;
            #[allow(deprecated)]
            let mut found = devices.find(|d| d.name().unwrap_or_default() == in_id);
            
            if found.is_none() {
                if let Some(card_name) = in_id.split("CARD=").nth(1).and_then(|s| s.split(',').next()) {
                    println!("⚠️ Exact match not found, trying fallback for card: '{}'", card_name);
                    let mut devices_retry = host.input_devices()?;
                    #[allow(deprecated)]
                    {
                        found = devices_retry.find(|d| d.name().unwrap_or_default().contains(card_name));
                    }
                }
            }

            match found {
                Some(d) => d,
                None => return Err(anyhow::anyhow!("Device not found: {}", in_id)),
            }
        };
        
        let out_device = host.default_output_device().ok_or_else(|| anyhow::anyhow!("No speaker found"))?;
        let in_config = in_device.default_input_config()?;
        let out_config = out_device.default_output_config()?;

        let in_sr = in_config.sample_rate() as f64;
        let out_sr = out_config.sample_rate() as f64;
        let in_format = in_config.sample_format();

        println!("🎙️  Audio: Opening {} ({}Hz, {:?})", in_id, in_sr, in_format);

        let (mut prod_in, mut cons_in) = HeapRb::<f32>::new(48000 * 2).split();
        let (mut prod_out, mut cons_out) = HeapRb::<f32>::new(48000 * 2).split();

        let in_ch = in_config.channels() as usize;
        
        let _in_stream = match in_format {
            cpal::SampleFormat::F32 => in_device.build_input_stream(&in_config.into(), move |data: &[f32], _| {
                for chunk in data.chunks(in_ch) { let _ = prod_in.try_push(chunk[0]); }
            }, |_| {}, None)?,
            cpal::SampleFormat::I16 => in_device.build_input_stream(&in_config.into(), move |data: &[i16], _| {
                for chunk in data.chunks(in_ch) { let _ = prod_in.try_push(chunk[0] as f32 / i16::MAX as f32); }
            }, |_| {}, None)?,
            _ => return Err(anyhow::anyhow!("Unsupported format")),
        };

        let out_ch = out_config.channels() as usize;
        let _out_stream = out_device.build_output_stream(&out_config.into(), move |data: &mut [f32], _| {
            for chunk in data.chunks_mut(out_ch) {
                let s = cons_out.try_pop().unwrap_or(0.0);
                for ch in chunk.iter_mut() { *ch = s; }
            }
        }, |_| {}, None)?;

        // DSP Thread
        std::thread::spawn(move || {
            let mut denoise = DenoiseState::new();
            let mut proc = Processor::new(&InitializationConfig { 
                num_capture_channels: 1, 
                num_render_channels: 1, 
                ..Default::default() 
            }).unwrap();
            
            proc.set_config(Config { 
                noise_suppression: Some(NoiseSuppression { suppression_level: NoiseSuppressionLevel::VeryHigh }), 
                echo_cancellation: if settings.aec_enabled { Some(webrtc_audio_processing::EchoCancellation {
                    suppression_level: webrtc_audio_processing::EchoCancellationSuppressionLevel::High,
                    stream_delay_ms: None, 
                    enable_delay_agnostic: true,
                    enable_extended_filter: true,
                }) } else { None },
                gain_control: if settings.agc_enabled { Some(webrtc_audio_processing::GainControl {
                    mode: webrtc_audio_processing::GainControlMode::AdaptiveDigital,
                    target_level_dbfs: 3,
                    compression_gain_db: 15,
                    enable_limiter: true,
                }) } else { None },
                enable_high_pass_filter: true, 
                enable_transient_suppressor: true, 
                ..Default::default() 
            });

            let params_in = SincInterpolationParameters { sinc_len: 256, f_cutoff: 0.95, interpolation: SincInterpolationType::Linear, window: WindowFunction::BlackmanHarris2, oversampling_factor: 256 };
            let params_out = SincInterpolationParameters { sinc_len: 256, f_cutoff: 0.95, interpolation: SincInterpolationType::Linear, window: WindowFunction::BlackmanHarris2, oversampling_factor: 256 };
            
            let mut res_in = SincFixedIn::<f32>::new(48000.0 / in_sr, 2.0, params_in, 480, 1).unwrap();
            let mut res_out = SincFixedIn::<f32>::new(out_sr / 48000.0, 2.0, params_out, 480, 1).unwrap();

            let mut dsp_buf = Vec::new();
            loop {
                let needed = res_in.input_frames_next();
                if cons_in.occupied_len() >= needed {
                    let mut chunk = vec![0.0f32; needed];
                    for s in chunk.iter_mut() { *s = cons_in.try_pop().unwrap(); }
                    if let Ok(res) = res_in.process(&[chunk], None) {
                        dsp_buf.extend_from_slice(&res[0]);
                        while dsp_buf.len() >= 480 {
                            let mut frame = dsp_buf.drain(0..480).collect::<Vec<_>>();
                            
                            // 1. Process Capture (Mic -> Clean)
                            let _ = proc.process_capture_frame(&mut frame);
                            
                            // 2. Extra Denoise
                            let mut clean = [0.0f32; 480];
                            denoise.process_frame(&mut clean, &frame);
                            
                            // 3. PTT Gate
                            let is_tx = state.is_transmitting.load(Ordering::Relaxed);
                            let output_frame = if is_tx { clean.to_vec() } else { vec![0.0; 480] };

                            // 4. Feed Render (Simulated Loopback)
                            let mut render_copy = output_frame.clone(); 
                            let _ = proc.process_render_frame(&mut render_copy);

                            // 5. Output to Speaker (Loopback)
                            if let Ok(res_o) = res_out.process(&[output_frame], None) {
                                for &s in &res_o[0] { 
                                    let _ = prod_out.try_push(s); 
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        _in_stream.play()?;
        _out_stream.play()?;
        Ok(Self { _streams: (_in_stream, _out_stream) })
    }
}

// Input Listener helper
pub fn start_input_listener(state: Arc<GlobalAudioState>, settings: Arc<Mutex<AudioSettings>>) {
    std::thread::spawn(move || {
        println!("⌨️  Global Input Listener started (rdev)");
        let callback = move |event: Event| {
            let (target_key, enabled) = {
                let s = settings.lock();
                (s.ptt_key, s.ptt_enabled)
            };

            if !enabled {
                state.is_transmitting.store(true, Ordering::Relaxed);
                return;
            }

            match event.event_type {
                EventType::KeyPress(key) => {
                    if key == target_key {
                        let prev = state.is_transmitting.swap(true, Ordering::Relaxed);
                        if !prev { print!("🎤 "); use std::io::Write; let _ = std::io::stdout().flush(); }
                    }
                },
                EventType::KeyRelease(key) => {
                    if key == target_key {
                        state.is_transmitting.store(false, Ordering::Relaxed);
                        print!("🔇 "); use std::io::Write; let _ = std::io::stdout().flush();
                    }
                },
                _ => {}
            }
        };
        if let Err(error) = listen(callback) { println!("❌ Input Error: {:?}", error); }
    });
}
