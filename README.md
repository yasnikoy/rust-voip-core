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