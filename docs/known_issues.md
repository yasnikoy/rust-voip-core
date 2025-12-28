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
