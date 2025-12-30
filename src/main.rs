use neandertal_voip_core::audio_service::{AudioSession, AudioSettings, GlobalAudioState, get_professional_device_list, start_input_listener};
use std::sync::{Arc, atomic::AtomicBool};
use parking_lot::Mutex;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    unsafe { libc::close(2); }
    env_logger::init();
    
    let settings = Arc::new(Mutex::new(AudioSettings::default()));
    
    // --- SHARED STATE ---
    let global_state = Arc::new(GlobalAudioState {
        is_transmitting: AtomicBool::new(true)
    });

    start_input_listener(global_state.clone(), settings.clone());

    println!("\n=== NEANDERTAL VOIP CORE AUDIO DEVICE LIST ===");
    let inputs = get_professional_device_list();
    for (i, dev) in inputs.iter().enumerate() { println!("{}. {}", i, dev.display_name); }
    println!("==============================\n");

    let mut _session: Option<AudioSession> = None;
    let mut last_id = String::new();

    loop {
        let (current_id, current_settings) = {
            let s = settings.lock();
            (s.input_device_id.clone(), s.clone())
        };

        if current_id != last_id {
            if _session.is_some() {
                println!("🛑 Closing old session...");
                _session = None;
            }
            std::thread::sleep(Duration::from_millis(1000));
            
            match AudioSession::create(&current_id, global_state.clone(), current_settings) {
                Ok(s) => { _session = Some(s); last_id = current_id; println!("✅ Active."); } 
                Err(e) => { 
                    println!("❌ Failed to open '{}': {:?}", current_id, e);
                    last_id = current_id; 
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
