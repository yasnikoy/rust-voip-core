//! Neandertal VoIP Core
//! 
//! High-performance audio and video streaming library.
//! Supports Linux (X11/Wayland) and Windows.

// Cross-platform modules
pub mod audio_service;
pub mod video_service;

// Linux-only: PipeWire Portal capture (Wayland 60+ FPS)
#[cfg(target_os = "linux")]
pub mod pipewire_capture;

// NVFBC capture (NVIDIA GPUs on Linux + Windows, 50-60+ FPS)
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod nvfbc_capture;

/// Platform detection helper
pub fn platform_info() -> &'static str {
    #[cfg(target_os = "linux")]
    { "Linux" }
    
    #[cfg(target_os = "windows")]
    { "Windows" }
    
    #[cfg(target_os = "macos")]
    { "macOS" }
    
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    { "Unknown" }
}

/// Get recommended capture backend for current platform
pub fn recommended_capture_backend() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        // Windows: xcap uses DXGI Desktop Duplication (60+ FPS)
        "xcap (DXGI)"
    }
    
    #[cfg(target_os = "linux")]
    {
        // Linux: Try NVFBC first, then PipeWire, then xcap
        if nvfbc_capture::is_nvfbc_available() {
            "nvfbc (NVIDIA GPU, 50+ FPS)"
        } else {
            "xcap (X11/Wayland, ~30 FPS)"
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        "xcap (SCStreamOutput, 60+ FPS)"
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "xcap (fallback)"
    }
}
