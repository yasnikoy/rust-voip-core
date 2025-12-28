# Rust Native VoIP Core

High-performance, low-latency audio pipeline using Rust and LiveKit.

## Core Architecture

- **Native I/O**: Direct hardware access via `cpal` (ASIO/WASAPI/CoreAudio/ALSA).
- **DSP Stack**: Dual-layer noise suppression (WebRTC + RNNoise).
- **Transport**: LiveKit Rust SDK (libwebrtc) for SFU-based distribution.
- **Dynamic Settings**: Thread-safe configuration system using `Arc<Mutex<AudioSettings>>`, ready for Tauri integration.

## Prerequisites

Required system libraries (Linux):
- libasound2-dev
- libpulse-dev
- libwebrtc-audio-processing-dev
- pkg-config
- libssl-dev

## Roadmap & Progress

### Level 1: Basic Management
- [x] Fixed 48kHz Processing Pipeline
- [x] Thread-safe Settings Infrastructure (Shared State)
- [x] Input/Output Device Selection
- [x] Digital Input Gain Control
- [x] Digital Output Gain Control

### Level 2: Transmission Control
- [x] Push-to-Talk (PTT) with Global Hotkeys
- [x] Voice Activity Detection (VAD) / Noise Gate
- [x] Configurable Gate Hold Time (Attack/Release)

### Level 3: Advanced DSP
- [x] Noise Suppression (RNNoise + WebRTC VeryHigh)
- [x] High-Pass Filter (HPF)
- [x] Transient Suppressor (Anti-Click/Pop)
- [x] Acoustic Echo Cancellation (AEC)

### Level 4: Quality & Dynamics
- [x] Automatic Gain Control (AGC)
- [x] Peak Limiter
- [x] Compressor

### Level 5: Spatial & Environmental
- [ ] 3D/Positional Audio (Panning/Distance)
- [ ] Dereverberation

## Usage

Run in release mode for optimal performance:

```bash
cargo run --release
```