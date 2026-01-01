//! Neandertal `VoIP` Core
//!
//! High-performance audio and video streaming library.
//! Supports Linux (X11/Wayland) and Windows.
//!
//! # Platform Support
//! - **Linux**: NVFBC, PipeWire Portal, xcap
//! - **Windows**: NVFBC, DXGI Desktop Duplication
//! - **macOS**: xcap (`SCStreamOutput`)

// Cross-platform modules
pub mod audio_service;
pub mod video_service;

// Linux-only: PipeWire Portal capture (Wayland 60+ FPS)
#[cfg(target_os = "linux")]
pub mod pipewire_capture;

// NVFBC capture (NVIDIA GPUs on Linux + Windows, 50-60+ FPS)
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod nvfbc_capture;

// GPU-accelerated color conversion (Linux with OpenGL)
#[cfg(target_os = "linux")]
pub mod gpu_color_convert;

// NVFBC with GPU color conversion (highest performance)
#[cfg(target_os = "linux")]
pub mod nvfbc_gpu_capture;

// Low-power NVFBC capture (for laptops like Acer E5-571G)
#[cfg(target_os = "linux")]
pub mod nvfbc_lowpower;

/// Platform detection helper (compile-time constant)
#[must_use]
pub const fn platform_info() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "Linux"
    }

    #[cfg(target_os = "windows")]
    {
        "Windows"
    }

    #[cfg(target_os = "macos")]
    {
        "macOS"
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "Unknown"
    }
}

/// Get recommended capture backend for current platform
///
/// # Returns
/// A static string describing the best capture backend for this platform
#[must_use]
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
