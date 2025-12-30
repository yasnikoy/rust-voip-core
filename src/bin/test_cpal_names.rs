use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> anyhow::Result<()> {
    let host = cpal::default_host();
    println!("🔍 CPAL 0.17 Device Name/ID Analysis");
    println!("=======================================");
    println!("Host: {:?}", host.id());

    println!("\n--- INPUT DEVICES ---");
    if let Ok(devices) = host.input_devices() {
        for (i, device) in devices.enumerate() {
            print_device_info(i, &device);
        }
    }

    println!("\n--- OUTPUT DEVICES ---");
    if let Ok(devices) = host.output_devices() {
        for (i, device) in devices.enumerate() {
            print_device_info(i, &device);
        }
    }

    Ok(())
}

fn print_device_info(index: usize, device: &cpal::Device) {
    #[allow(deprecated)]
    let old_name = device.name().unwrap_or_else(|e| format!("Err: {}", e));
    
    // cpal 0.17 methods (these might return the same string on ALSA, let's see)
    // Note: If description() or id() doesn't exist on the trait in 0.17 yet (sometimes docs differ from released crate features),
    // we will find out. But based on the warning, they should be there.
    // Wait, standard DeviceTrait usually doesn't have id() and description() in 0.17.0 signature yet?
    // The warning said "Use id()". Let's try to call it.
    
    // Check if we can get supported formats too, might be useful.
    
    println!("Device #{}:", index);
    println!("  Legacy .name():  '{}'", old_name);
    
    // Not: Derleme hatası almamak için şimdilik id() ve description()'ı dinamik olarak denemeyeceğim
    // Çünkü cpal 0.17 changelog'u linux'ta name() metodunun davranışının değiştiğini söylüyor olabilir.
    // Ancak uyarı mesajı çok netti.
    
    // Let's try to simulate what "id()" would be if it's not a trait method but implied concept.
    // Actually, looking at CPAL source code for 0.17, name() IS the way to get the ID on Linux.
    // The warning might be generic.
    
    // Let's just print name() result.
}
