# InputMode Design: Phase 1 — Scaling & Axis Assignment

**Problem:** The `cube.rs` and `asteroids3d.rs` examples currently hardcode scaling factors and axis mappings (`p.tx += m.translation[0] * 3.0`, `p.rx += m.rotation[0] / E`, etc.). These values are ad-hoc and don't generalize across use cases:

- **Object manipulation** (cube): You want the object to move smoothly in the world, with intuitive rotation.
- **Camera control** (asteroids): You want the camera to respond like a player's viewpoint — forward/back motion, strafe, yaw/pitch/roll — with different sensitivity.

The same device, same `NormalizedMotion` stream, but different *semantics* and scaling requirements.

**Goal:** Design a lightweight `InputMode` enum that:
1. Makes common cases (object vs camera) trivial — pass a preset.
2. Doesn't expose axis mapping yet (defer Phase 1.5).
3. Allows per-example tuning without recompilation (Phase 2).
4. Lives in `src/input_mode.rs` so examples can `use spaceball_rs::InputMode`.

---

## Architecture

### New module: `src/input_mode.rs`

```rust
/// Semantic interpretation of 6DOF motion and sensitivity tuning.
#[derive(Debug, Clone)]
pub enum InputMode {
    /// Object manipulation: device axes map directly to world/object-space axes.
    /// Typical use: rotating/translating a 3D model in front of a fixed camera.
    ///
    /// - Device forward (–Z) → object moves forward
    /// - Device twist → object rotates around its up axis (or world Z)
    /// - All axes move in world-space or object-space (not camera-relative)
    ObjectManipulation {
        /// Scale factor for translation.
        /// `output_velocity = normalized_input * scale`
        /// Units: world units per second at max device deflection.
        /// Typical range: 0.1–10.0 (smaller = slower, larger = faster).
        translation_scale: f32,

        /// Scale factor for rotation.
        /// `output_angular_velocity = normalized_input * scale`
        /// Units: radians per second at max device deflection.
        /// Typical range: 0.5–5.0 (rad/s).
        rotation_scale: f32,
    },

    /// Camera/viewpoint control: device axes map to *camera-relative* motion.
    /// Typical use: first-person games where you move and look around.
    ///
    /// - Device forward → camera moves forward (in whatever direction it's facing)
    /// - Device right → camera strafes right
    /// - Device up → camera moves up (world-space vertical)
    /// - Device pitch → camera pitches (looks up/down)
    /// - Device yaw → camera yaws (turns left/right)
    /// - Device roll → camera rolls (horizon tilts, if supported)
    CameraControl {
        /// Scale factor for translation velocity.
        /// Units: world units per second at max device deflection.
        /// Typical range: 0.1–10.0.
        translation_scale: f32,

        /// Scale factor for rotation rate.
        /// Units: radians per second at max device deflection.
        /// Typical range: 0.5–5.0 (rad/s).
        rotation_scale: f32,
    },
}

impl InputMode {
    /// Preset: object manipulation with sensible defaults.
    pub fn object_manipulation_default() -> Self {
        InputMode::ObjectManipulation {
            translation_scale: 3.0,
            rotation_scale: 2.0,
        }
    }

    /// Preset: camera control with sensible defaults.
    pub fn camera_control_default() -> Self {
        InputMode::CameraControl {
            translation_scale: 5.0,
            rotation_scale: 1.5,
        }
    }
}

/// Scaled motion output: ready to apply to the scene.
///
/// Call [`process`] to convert `NormalizedMotion` + `InputMode` into `ScaledMotion`.
#[derive(Debug, Clone)]
pub struct ScaledMotion {
    /// Translation velocity, in world units per second.
    /// For object mode: add to object position.
    /// For camera mode: add to camera position (in camera direction).
    pub translation: [f32; 3],

    /// Rotation angular velocity, in radians per second.
    /// For object mode: add to object rotation angles.
    /// For camera mode: add to camera pitch/yaw/roll.
    pub rotation: [f32; 3],
}

/// Apply scaling and semantic interpretation to normalized device input.
///
/// # Example: Object manipulation
/// ```ignore
/// let mode = InputMode::ObjectManipulation {
///     translation_scale: 2.0,
///     rotation_scale: 1.5,
/// };
/// let motion = NormalizedMotion {
///     translation: [0.5, 0.0, 0.0],
///     rotation: [0.0, 0.0, 0.0],
/// };
/// let scaled = process(&motion, &mode);
/// assert_eq!(scaled.translation[0], 1.0); // 0.5 * 2.0
/// ```
pub fn process(input: &crate::NormalizedMotion, mode: &InputMode) -> ScaledMotion {
    let (tx_scale, rx_scale) = match mode {
        InputMode::ObjectManipulation {
            translation_scale,
            rotation_scale,
        } => (*translation_scale, *rotation_scale),
        InputMode::CameraControl {
            translation_scale,
            rotation_scale,
        } => (*translation_scale, *rotation_scale),
    };

    ScaledMotion {
        translation: [
            input.translation[0] * tx_scale,
            input.translation[1] * tx_scale,
            input.translation[2] * tx_scale,
        ],
        rotation: [
            input.rotation[0] * rx_scale,
            input.rotation[1] * rx_scale,
            input.rotation[2] * rx_scale,
        ],
    }
}
```

### Update `src/lib.rs`

Add to the module declarations:

```rust
mod input_mode;
pub use input_mode::{InputMode, ScaledMotion, process};
```

Make sure these are public re-exports so examples can do `use spaceball_rs::InputMode`.

---

## Usage Pattern: cube.rs (ObjectManipulation)

**Before:**
```rust
// Hardcoded magic numbers; scales vary per example
p.tx += m.translation[0] * 3.0;
p.ty += m.translation[1] * 3.0;
p.tz += m.translation[2] * 3.0;
p.rx += m.rotation[0] / std::f32::consts::E;  // ← What is E? Why divide?
p.ry += m.rotation[1] / std::f32::consts::E;
p.rz += m.rotation[2] / std::f32::consts::E;
```

**After:**
```rust
use spaceball_rs::{DeviceEvent, SixDofDevice, InputMode, process};

let mode = InputMode::object_manipulation_default();

// In the background thread's motion handler:
for event in device.events() {
    Ok(DeviceEvent::Motion(m)) => {
        let scaled = process(&m, &mode);
        let mut p = pose_bg.lock().unwrap();
        p.tx += scaled.translation[0] * dt;
        p.ty += scaled.translation[1] * dt;
        p.tz += scaled.translation[2] * dt;
        p.rx += scaled.rotation[0] * dt;
        p.ry += scaled.rotation[1] * dt;
        p.rz += scaled.rotation[2] * dt;
    }
    // ...
}
```

**Key differences:**
- `InputMode::object_manipulation_default()` is self-documenting.
- `process()` applies the scaling, so the example code is cleaner.
- `dt` (delta-time) is now explicit, matching "per-second" semantics of `NormalizedMotion`.

---

## Usage Pattern: asteroids3d.rs (CameraControl)

**Before:**
```rust
// Spaceball-specific, hardcoded, inconsistent with cube.rs
// (This part is omitted from your uploaded code, but presumably similar)
```

**After:**
```rust
use spaceball_rs::{DeviceEvent, SixDofDevice, InputMode, process};

let mode = InputMode::camera_control_default();

// In the background thread:
for event in device.events() {
    Ok(DeviceEvent::Motion(m)) => {
        let scaled = process(&m, &mode);
        // Asteroids logic: apply scaled motion to camera
        let mut state = game_state.lock().unwrap();
        state.camera_translation = scaled.translation;
        state.camera_rotation = scaled.rotation;
    }
    // ...
}
```

---

## Integration Steps

### Step 1: Create `src/input_mode.rs`

Copy the module code above into a new file.

### Step 2: Update `src/lib.rs`

Add:
```rust
mod input_mode;
pub use input_mode::{InputMode, ScaledMotion, process};
```

Verify the module is declared early (before or after device modules doesn't matter).

### Step 3: Update `examples/cube.rs`

1. Import: `use spaceball_rs::{..., InputMode, process};`
2. Create mode: `let mode = InputMode::object_manipulation_default();`
3. In the background thread, replace hardcoded scales with `process(&m, &mode)`.
4. **For now:** keep magic default values in `InputMode::object_manipulation_default()`. These will be tunable in Phase 2.

### Step 4: Update `examples/asteroids3d.rs`

Same pattern, but use `InputMode::camera_control_default()`.

### Step 5: Verify

```bash
cargo check --examples
cargo build --examples
cargo run --example cube
cargo run --example asteroids3d
```

Behavior should be *roughly* the same as before (some fine-tuning may be needed if the defaults don't match your old magic numbers exactly).

---

## Phase 2 Preview: Live Tuning (Future)

Once Phase 1 is solid, Phase 2 adds:

1. **Config struct with per-axis overrides:**
   ```rust
   pub struct FullConfig {
       pub mode: InputMode,
       pub tx_scale_override: Option<f32>,
       pub ty_scale_override: Option<f32>,
       // ... per-axis scaling
   }
   ```

2. **Config file / environment loading:**
   ```rust
   let config = load_config_from_file("spacemouse.toml")?;
   // or env vars for quick tweaks
   ```

3. **Egui sliders in examples:**
   - cube.rs: sidebar with translation/rotation scale sliders
   - asteroids3d.rs: settings menu with tuning sliders

4. **Config save/restore** so users don't have to re-tune every session.

---

## Open Questions / Refinements

### Q: Should `InputMode` also include dead zones?

**Answer (Phase 1):** No. Dead zones are per-device hardware tuning; they belong in device initialization, not here. Phase 2 might add them if needed.

### Q: What about different scales for translation vs rotation *per axis*?

**Answer (Phase 1):** Defer. The single `translation_scale` and `rotation_scale` cover the main cases. Phase 2 can add per-axis overrides if asteroids3d needs Y-translation scaled differently from X/Z.

### Q: Should we provide presets beyond these two?

**Answer (Phase 1):** No. Object vs camera are the main two. Phase 2 can add specialized presets (e.g., "spaceship autopilot", "robotic arm") if use cases emerge.

### Q: What about the reference-frame semantics (world vs camera-relative)?

**Answer (Phase 1):** The enum *name* (`CameraControl` vs `ObjectManipulation`) documents the intent, but the actual transformation logic is deferred. For now, both modes use the same axis passthrough; the difference is how the application *interprets* the scaled values. In Phase 2, we might add `apply_to_camera()` / `apply_to_object()` helper functions that do the math.

---

## Acceptance Criteria

- ✅ `src/input_mode.rs` compiles and is re-exported from `lib.rs`
- ✅ `cube.rs` uses `InputMode::object_manipulation_default()` + `process()`
- ✅ `asteroids3d.rs` uses `InputMode::camera_control_default()` + `process()`
- ✅ Both examples produce behavior equivalent to before (or very close)
- ✅ `cargo check --examples` passes
- ✅ No additional dependencies added
- ✅ Code is self-documenting (clear intent from names and doc comments)
