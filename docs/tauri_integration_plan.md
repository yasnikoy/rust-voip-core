# Tauri Desktop Application Integration Plan

## Overview

Convert existing neandertal-voip-core Rust library into a desktop application using Tauri framework for screen sharing testing without browser limitations.

## Current State

**Existing Infrastructure:**
- Production-ready Rust library (neandertal-voip-core)
- Screen capture: NVFBC, PipeWire, xcap
- Audio capture: cpal with processing (nnnoiseless, webrtc-audio-processing)
- LiveKit SDK integration
- GStreamer video pipeline
- Platform support: Linux (primary), Windows, macOS

**Code Quality:**
- 12 unit tests passing
- 3 doc tests passing
- 9 minor clippy warnings
- Zero critical issues
- Clean code patterns applied

## Architecture Decision - CONFIRMED

**Selected: Option A - Separate Tauri Project**

**Project Structure:**
```
~/Docker/dcts/
├── dcts-shipping-main/        # Core Rust library (existing)
└── dcts-tauri/                # New Tauri desktop app
    ├── src-tauri/             # Rust backend
    │   ├── Cargo.toml         # Dependencies (includes neandertal-voip-core)
    │   └── src/
    │       ├── main.rs
    │       └── commands.rs
    ├── src/                   # React frontend
    │   ├── App.tsx
    │   ├── components/
    │   └── hooks/
    └── package.json
```

**Core Library Integration:**
Core library will be used via path dependency in Cargo.toml:
```toml
[dependencies]
neandertal-voip-core = { path = "../dcts-shipping-main" }
```

**Rationale:**
- Separation of concerns (library vs application)
- Independent versioning
- Core library reusable for other projects
- Cleaner git history
- Easier package distribution

## Technical Stack - CONFIRMED

**Backend:**
- Tauri 2.0 (stable, latest version)
- Rust 2021 edition
- neandertal-voip-core (existing library)
- Tauri commands for IPC

**Frontend:**
- React 18
- TypeScript
- Vite (build tool)
- Simple UI for testing purposes

**Build Tool:**
- Vite (recommended by Tauri 2.0)

**Target Platforms:**
- Linux (primary development)
- Windows (cross-compilation/testing)
- macOS (cross-compilation/testing)

**Distribution Format:**
- Linux: .deb and .AppImage
- Windows: .exe and .msi installer
- macOS: .dmg bundle

All platforms will support single-click installation and launch.

## Core Tauri Commands

Commands to expose from Rust to frontend:

**Screen Capture:**
```rust
capture_start(backend: String, resolution: (u32, u32))
capture_stop()
capture_get_stats() -> CaptureStats
```

**LiveKit Integration:**
```rust
livekit_connect(url: String, token: String)
livekit_disconnect()
livekit_publish_track()
livekit_get_connection_stats() -> ConnectionStats
```

**Device Management:**
```rust
get_monitors() -> Vec<MonitorInfo>
get_audio_devices() -> Vec<AudioDeviceInfo>
set_audio_device(device_id: String)
```

**Configuration:**
```rust
get_available_backends() -> Vec<String>
get_platform_info() -> PlatformInfo
set_capture_settings(settings: CaptureSettings)
```

## Frontend Components

**Required UI Elements:**

1. Connection Panel
   - LiveKit URL input
   - Token input
   - Connect/Disconnect button

2. Capture Control
   - Backend selector (NVFBC/PipeWire/xcap)
   - Monitor/screen selector
   - Resolution selector
   - Start/Stop capture button

3. Statistics Display
   - Current FPS
   - Bitrate
   - Resolution
   - CPU usage
   - Frame drops
   - Connection quality

4. Settings Panel
   - Audio device selection
   - Video quality presets
   - Advanced options (optional)

5. Log Viewer
   - Real-time log display
   - Log level filter

## Implementation Phases

**Phase 1: Project Setup**
1. Create Tauri project structure (based on user choice)
2. Configure workspace dependencies
3. Setup basic Tauri window
4. Verify build process

**Phase 2: Backend Commands**
1. Implement Tauri command wrappers
2. Add state management for capture sessions
3. Implement error handling for commands
4. Add logging infrastructure

**Phase 3: Frontend Basic UI**
1. Create connection panel
2. Implement monitor selection
3. Add basic capture controls
4. Setup event listeners

**Phase 4: LiveKit Integration**
1. Integrate existing LiveKit code
2. Add track publishing
3. Implement connection state management
4. Add reconnection logic

**Phase 5: Statistics & Monitoring**
1. Implement FPS counter
2. Add bitrate monitoring
3. CPU usage tracking
4. Frame drop detection
5. Real-time stats display

**Phase 6: Testing & Polish**
1. End-to-end testing
2. Error handling refinement
3. UI/UX improvements
4. Performance optimization
5. Documentation

## Configuration Decisions - CONFIRMED

**Tauri Version:**
- Tauri 2.0 (stable)

**Window Configuration:**
- Single window application
- Fixed or resizable window (to be decided in implementation)
- Typical size: 1200x800px

**Connection Behavior:**
- Manual connection only
- User enters LiveKit URL and token each time
- No credential storage (security + flexibility for testing)
- Quick reconnect button for repeated tests

**Logging:**
- Console output during development
- File logging in production builds
- Log viewer component in UI (optional, can be added later)

**Distribution:**
- Production bundles for all platforms
- Single-click installation
- No complex setup required
- Platform-specific installers:
  - Linux: .deb (Ubuntu/Debian) + .AppImage (universal)
  - Windows: .exe installer + portable .exe
  - macOS: .dmg bundle

## Security Considerations

**Tauri Allowlist Configuration:**
```json
{
  "allowlist": {
    "all": false,
    "fs": {
      "scope": ["$APPDATA/*"]
    },
    "shell": {
      "open": false
    },
    "window": {
      "all": false,
      "create": true,
      "center": true
    }
  }
}
```

**Environment Variables:**
- Store LiveKit credentials securely
- Use Tauri's secure storage API
- Never expose secrets in frontend

## Testing Strategy

**Rust Backend Tests:**
- Unit tests for command handlers
- Integration tests for LiveKit flows
- Mock tests for hardware dependencies

**Frontend Tests:**
- Component tests (if React/Vue)
- E2E tests with Tauri driver
- Manual testing on target platforms

**Performance Tests:**
- FPS benchmarks
- Memory leak detection
- Long-running session tests

## Documentation Requirements

**Developer Docs:**
- Setup instructions
- Architecture overview
- Command reference
- Troubleshooting guide

**User Docs:**
- Installation guide
- Quick start tutorial
- Configuration reference
- Known limitations

## Confirmed Implementation Approach

**All Key Decisions Made:**
1. Project Structure: Separate Tauri project
2. Tauri Version: 2.0 (stable)
3. Frontend: React 18 + TypeScript
4. Platforms: Linux, Windows, macOS (cross-platform)
5. Distribution: Production bundles for all platforms
6. Window: Single window application
7. Connection: Manual (test-focused)

## Next Steps - Ready to Begin

**Phase 1: Project Setup (Immediate)**
1. Create new Tauri 2.0 project in ~/Docker/dcts/dcts-tauri
2. Configure path dependency to neandertal-voip-core
3. Setup React + TypeScript + Vite
4. Verify basic window launches
5. Test cross-platform build configuration

**Phase 2: Backend Commands (Next)**
1. Implement platform detection command
2. Add monitor enumeration
3. Create capture start/stop commands
4. Setup LiveKit connection commands
5. Add error handling and state management

**Phase 3: Frontend UI (After Backend)**
1. Create connection panel
2. Add monitor selector
3. Implement capture controls
4. Build statistics display
5. Add basic styling

**Phase 4: Integration & Testing**
1. Connect all components
2. Test on Linux
3. Build for Windows and macOS
4. Verify installers work
5. Document any platform-specific quirks

## Ready to Proceed?

All architectural decisions confirmed. Ready to start implementation when you give the go-ahead.

## Estimated Timeline

Based on scope:
- Minimal viable app (Phases 1-3): 2-3 days
- Full featured app (Phases 1-5): 5-7 days
- Production ready (All phases): 10-14 days

Timeline assumes part-time development (2-4 hours/day).
