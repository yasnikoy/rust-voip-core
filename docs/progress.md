# Project Roadmap

## Level 1: Basic Audio Management (Refactoring)
- [x] Improve ALSA device enumeration to handle duplicates (hw, plughw, default)
- [x] Implement friendly name parsing for Linux audio devices
- [x] Prioritize plughw over hw interfaces for better format compatibility
- [x] Implement strict format checking (f32/i16) before stream creation

## Level 2: Transmission Control
- [x] Implement Push-to-Talk (PTT) mechanism
- [x] Add global hotkey support for PTT

## Level 3: Advanced DSP
- [x] Add Acoustic Echo Cancellation (AEC)

## Level 4: Quality & Dynamics
- [x] Implement Automatic Gain Control (AGC)
- [x] Add Peak Limiter
- [x] Add Compressor

## Level 5: Spatial & Environmental
- [ ] Implement 3D/Positional Audio
- [ ] Add Dereverberation support

## Level 6: Dependency Modernization (Planned)
**Goal:** Update core audio libraries to latest stable versions for performance and stability.
1.  **Audio Backend (`cpal` 0.15 -> 0.17)**:
    - [ ] Update crate version.
    - [ ] Adapt to any breaking changes in `StreamConfig` or device enumeration.
    - [ ] Verify ALSA/PulseAudio compatibility on Linux.
2.  **DSP Engine (`webrtc-audio-processing` 0.3 -> 0.5)**:
    - [ ] Update crate.
    - [ ] Review changes in `AudioProcessing` config structures (Echo Cancellation, Gain Control).
    - [ ] Validate DSP quality (no regressions in noise/echo).
3.  **Resampling (`rubato` 0.14 -> Latest)**:
    - [ ] Update crate.
    - [ ] Check for API shifts (Async vs Sync resampler initialization).
4.  **Utilities & Cleanup**:
    - [ ] Update `ringbuf` (0.3 -> 0.4+) if needed for compatibility.
    - [ ] Run `cargo audit` to check for security advisories.