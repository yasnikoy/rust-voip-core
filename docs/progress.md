# Project Roadmap

## Level 1: Basic Audio Management (Refactoring)
- [x] Improve ALSA device enumeration to handle duplicates (hw, plughw, default)
- [x] Implement friendly name parsing for Linux audio devices
- [x] Prioritize plughw over hw interfaces for better format compatibility
- [x] Implement strict format checking (f32/i16) before stream creation

## Level 2: Transmission Control
- Implement Push-to-Talk (PTT) mechanism
- Add global hotkey support for PTT

## Level 3: Advanced DSP
- Add Acoustic Echo Cancellation (AEC)

## Level 4: Quality & Dynamics
- Implement Automatic Gain Control (AGC)
- Add Peak Limiter
- Add Compressor

## Level 5: Spatial & Environmental
- Implement 3D/Positional Audio
- Add Dereverberation support
