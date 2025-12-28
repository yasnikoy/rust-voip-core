# Known Issues & Limitations

## Linux Audio Device Switching
**Issue:** Switching input devices at runtime fails when using `cpal` with ALSA backend while PipeWire/PulseAudio is active.
**Symptoms:** 
- Application starts successfully with "System Default" (PipeWire).
- Attempting to switch to a specific hardware device (e.g., `sysdefault:CARD=...`) fails with `Device not found`.
- Direct hardware devices disappear from enumeration after a stream is opened via `default`.
**Cause:** 
- PipeWire/PulseAudio acquires exclusive access to ALSA hardware devices.
- `cpal` (via ALSA) cannot access the hardware device because the sound server has locked it.
- Closing the `cpal` stream does not immediately release the device resource in a way that allows instant reconfiguration.
**Workaround:** 
- Users on Linux should prefer using "System Default" and manage device selection via OS settings (Pavucontrol/Gnome Settings).
- Alternatively, launch the application with the specific device ID targeting the hardware directly, bypassing the sound server initially.

## Acoustic Echo Cancellation (AEC) Testing
**Issue:** AEC effectiveness is difficult to verify in the current local loopback mode.
**Details:**
- AEC relies on a reference signal (typically incoming network audio) to cancel out echoes from the speaker.
- In loopback mode, the reference signal is the microphone's own output. While this creates a feedback loop that AEC might partially suppress, it doesn't simulate real-world echo conditions (network delay + acoustic reflection).
- "Clean" audio in loopback mode might indicate that AEC is either not aggressive enough or the latency estimation is mismatched for the immediate local loop.
**Future Action:**
- Verify and tune AEC performance (delay estimation, suppression level) once network transmission (Level 6+) is implemented and a true "sender-receiver" test environment is available.