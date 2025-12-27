# Rust Native VoIP Core

High-performance, low-latency audio pipeline using Rust and LiveKit.

## Features

- Native audio capture and playback via cpal.
- Double-layer noise suppression: WebRTC Audio Processing + RNNoise (nnnoiseless).
- High-pass filtering and transient suppression.
- LiveKit Rust SDK for SFU transport.
- Fixed 48kHz processing pipeline.

## Prerequisites

Required system libraries (Linux):
- libasound2-dev
- libpulse-dev
- libwebrtc-audio-processing-dev
- pkg-config
- libssl-dev

## Configuration

Create a `.env` file in the root directory:

```ini
LIVEKIT_URL=ws://localhost:7880
LIVEKIT_TOKEN=your_access_token
```

## Usage

Run in release mode for optimal audio performance:

```bash
cargo run --release
```

## Roadmap

### Level 1: Basic Management
- Input/Output Device Selection: Ability to choose specific hardware interfaces.
- Digital Gain Control: Manual adjustment of input/output volume levels.

### Level 2: Transmission Control
- Push-to-Talk (PTT): Global hotkey integration for manual transmission control.
- Voice Activity Detection (VAD): Intelligent microphone activation based on signal analysis.

### Level 3: Advanced DSP (In Progress)
- Noise Suppression (NS): Multi-stage removal of background noise (RNNoise + WebRTC). [Implemented]
- High-Pass Filter (HPF): Removal of low-frequency electrical hum and mechanical vibrations. [Implemented]
- Acoustic Echo Cancellation (AEC): Preventing speaker output from re-entering the microphone.

### Level 4: Quality & Dynamics
- Automatic Gain Control (AGC): Dynamic normalization of varied participant volume levels.
- Peak Limiter: Prevention of digital clipping and audio distortion during loud transients.

### Level 5: Spatial & Environmental
- 3D/Positional Audio: Stereo panning and distance attenuation for spatial immersion.
- Dereverberation: Reduction of room acoustic reflections for a studio-quality sound.
