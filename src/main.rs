use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use parking_lot::Mutex;
use ringbuf::{HeapRb, Rb};
use nnnoiseless::DenoiseState;
use webrtc_audio_processing::{Processor, InitializationConfig, Config, NoiseSuppression, NoiseSuppressionLevel};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct AudioSettings {
    input_device_name: String,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { input_device_name: "default".to_string() }
    }
}

struct AudioSession {
    _streams: (cpal::Stream, cpal::Stream),
}

impl AudioSession {
    fn create(host: &cpal::Host, in_name: &str) -> anyhow::Result<Self> {
        let in_device = if in_name == "default" {
            host.default_input_device().ok_or_else(|| anyhow::anyhow!("No default mic"))?
        } else {
            host.input_devices()? 
                .find(|d| d.name().unwrap_or_default() == in_name)
                .ok_or_else(|| anyhow::anyhow!("Device not found: {}", in_name))?
        };
        
        let out_device = host.default_output_device().expect("No speaker");
        let in_config = in_device.default_input_config()?;
        let out_config = out_device.default_output_config()?;

        let in_sr = in_config.sample_rate().0 as f64;
        let out_sr = out_config.sample_rate().0 as f64;
        let in_format = in_config.sample_format();

        println!("🎙️  Opening: {} ({}Hz, {:?})", in_name, in_sr, in_format);

        let (mut prod_in, mut cons_in) = HeapRb::<f32>::new(in_sr as usize * 2).split();
        let (mut prod_out, mut cons_out) = HeapRb::<f32>::new(out_sr as usize * 2).split();

        let in_ch = in_config.channels() as usize;
        let _in_stream = match in_format {
            cpal::SampleFormat::F32 => in_device.build_input_stream(&in_config.into(), move |data: &[f32], _| {
                for chunk in data.chunks(in_ch) { let _ = prod_in.push(chunk[0]); }
            }, |_| {}, None)?,
            cpal::SampleFormat::I16 => in_device.build_input_stream(&in_config.into(), move |data: &[i16], _| {
                for chunk in data.chunks(in_ch) { let _ = prod_in.push(chunk[0] as f32 / i16::MAX as f32); }
            }, |_| {}, None)?,
            _ => return Err(anyhow::anyhow!("Unsupported format")),
        };

        let out_ch = out_config.channels() as usize;
        let _out_stream = out_device.build_output_stream(&out_config.into(), move |data: &mut [f32], _| {
            for chunk in data.chunks_mut(out_ch) {
                let s = cons_out.pop().unwrap_or(0.0);
                for ch in chunk.iter_mut() { *ch = s; }
            }
        }, |_| {}, None)?;

        std::thread::spawn(move || {
            let mut denoise = DenoiseState::new();
            let mut proc = Processor::new(&InitializationConfig { num_capture_channels: 1, num_render_channels: 1, ..Default::default() }).unwrap();
            proc.set_config(Config { noise_suppression: Some(NoiseSuppression { suppression_level: NoiseSuppressionLevel::VeryHigh }), enable_high_pass_filter: true, enable_transient_suppressor: true, ..Default::default() });

            // Define params twice to avoid clone error
            let params_in = SincInterpolationParameters { sinc_len: 256, f_cutoff: 0.95, interpolation: SincInterpolationType::Linear, window: WindowFunction::BlackmanHarris2, oversampling_factor: 256 };
            let params_out = SincInterpolationParameters { sinc_len: 256, f_cutoff: 0.95, interpolation: SincInterpolationType::Linear, window: WindowFunction::BlackmanHarris2, oversampling_factor: 256 };
            
            let mut res_in = SincFixedIn::<f32>::new(48000.0 / in_sr, 2.0, params_in, 480, 1).unwrap();
            let mut res_out = SincFixedIn::<f32>::new(out_sr / 48000.0, 2.0, params_out, 480, 1).unwrap();

            let mut dsp_buf = Vec::new();
            loop {
                let needed = res_in.input_frames_next();
                if cons_in.len() >= needed {
                    let mut chunk = vec![0.0f32; needed];
                    for s in chunk.iter_mut() { *s = cons_in.pop().unwrap(); }
                    if let Ok(res) = res_in.process(&[chunk], None) {
                        dsp_buf.extend_from_slice(&res[0]);
                        while dsp_buf.len() >= 480 {
                            let mut frame = dsp_buf.drain(0..480).collect::<Vec<_>>();
                            let _ = proc.process_capture_frame(&mut frame);
                            let mut clean = [0.0f32; 480];
                            denoise.process_frame(&mut clean, &frame);
                            if let Ok(res_o) = res_out.process(&[clean.to_vec()], None) {
                                for &s in &res_o[0] { let _ = prod_out.push(s); }
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

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    unsafe { libc::close(2); } // ALSA karmaşasını gizle
    
    env_logger::init();
    let host = cpal::default_host();
    let settings = Arc::new(Mutex::new(AudioSettings::default()));

    println!("\n=== DCTS AUDIO DEVICE LIST ===");
    // CPAL üzerinden gerçek ve temiz bir liste oluştur
    let mut available_mics = Vec::new();
    available_mics.push("default".to_string());
    
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                let l = name.to_lowercase();
                if !l.contains("surround") && !l.contains("dmix") && !l.contains("dsnoop") && !l.contains("null") && name != "default" {
                    available_mics.push(name);
                }
            }
        }
    }
    available_mics.dedup();

    for (i, name) in available_mics.iter().enumerate() {
        println!("{}. {}", i, name);
    }
    println!("==============================\n");

    let alt_mic_name = available_mics.iter().find(|&n| n != "default" && n != "pipewire").cloned().unwrap_or("default".to_string());

    let mut _session: Option<AudioSession> = None;
    let mut last_name = String::new();
    let start_time = Instant::now();

    loop {
        let current_name = settings.lock().input_device_name.clone();
        if current_name != last_name {
            println!("🔄 Switching to: {}", current_name);
            _session = None;
            std::thread::sleep(Duration::from_millis(500));
            match AudioSession::create(&host, &current_name) {
                Ok(s) => { _session = Some(s); last_name = current_name; println!("✅ Active."); } 
                Err(e) => { println!("❌ Failed: {}. Retrying default.", e); settings.lock().input_device_name = "default".to_string(); }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
        if start_time.elapsed() > Duration::from_secs(10) && settings.lock().input_device_name == "default" {
            println!("⏰ Switching to: {}", alt_mic_name);
            settings.lock().input_device_name = alt_mic_name.clone();
        }
    }
}