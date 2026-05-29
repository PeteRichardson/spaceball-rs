# Input Mode Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `InputMode` and `process()` to the library so examples declare their scaling intent in self-documenting code rather than magic numbers, with delta-time applied for correct per-second semantics.

**Architecture:** A new `src/input_mode.rs` module exports `InputMode` (enum with `ObjectManipulation` and `CameraControl` variants), `ScaledMotion` (output struct), and a free function `process()` that multiplies `NormalizedMotion` by the mode's scale factors. Both examples adopt `process()` and add `std::time::Instant` for `dt`. `asteroids3d.rs` keeps its existing axis remapping (Z-negation, rotation axis reordering) around the `process()` call.

**Tech Stack:** Rust std only — `std::time::Instant`, existing `NormalizedMotion` from `src/lib.rs`.

---

### Task 1: Create `src/input_mode.rs` skeleton and wire into lib.rs

**Files:**
- Create: `src/input_mode.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/input_mode.rs` with stubs**

Write types and function signatures with `todo!()` bodies. The `todo!()` lets tests compile and then fail at runtime (the correct TDD "red" state).

```rust
/// Semantic interpretation of 6DOF motion and sensitivity tuning.
#[derive(Debug, Clone)]
pub enum InputMode {
    /// Object manipulation: device axes map directly to world/object-space axes.
    /// Typical use: rotating/translating a 3D model in front of a fixed camera.
    ObjectManipulation {
        /// Scale factor for translation. Units: world units per second at max deflection.
        /// Typical range: 0.1–10.0.
        translation_scale: f32,
        /// Scale factor for rotation. Units: radians per second at max deflection.
        /// Typical range: 0.5–5.0.
        rotation_scale: f32,
    },
    /// Camera/viewpoint control: device axes map to camera-relative motion.
    /// Typical use: first-person games where you move and look around.
    CameraControl {
        /// Scale factor for translation velocity. Units: world units per second at max deflection.
        /// Typical range: 0.1–10.0.
        translation_scale: f32,
        /// Scale factor for rotation rate. Units: radians per second at max deflection.
        /// Typical range: 0.5–5.0.
        rotation_scale: f32,
    },
}

impl InputMode {
    /// Preset: object manipulation with sensible defaults.
    pub fn object_manipulation_default() -> Self {
        todo!()
    }

    /// Preset: camera control with sensible defaults.
    pub fn camera_control_default() -> Self {
        todo!()
    }
}

/// Scaled motion output. Apply per frame as: `pos += translation * dt`.
#[derive(Debug, Clone)]
pub struct ScaledMotion {
    /// Translation velocity in world units per second.
    pub translation: [f32; 3],
    /// Rotation angular velocity in radians per second.
    pub rotation: [f32; 3],
}

/// Apply scaling to normalized device input.
///
/// Multiply the result's fields by `dt` (seconds since last event) before
/// accumulating into position/rotation. `ScaledMotion` carries per-second rates,
/// not per-frame deltas.
pub fn process(input: &crate::NormalizedMotion, mode: &InputMode) -> ScaledMotion {
    todo!()
}
```

- [ ] **Step 2: Add module declaration and re-exports to `src/lib.rs`**

After the two existing `mod` lines (`mod spaceball;` / `mod spaceorb;`), add:

```rust
mod input_mode;
pub use input_mode::{InputMode, ScaledMotion, process};
```

- [ ] **Step 3: Verify compilation (no tests yet)**

```bash
cargo check
```

Expected: compiles cleanly. Warnings about unreachable `todo!()` are fine.

---

### Task 2: Write failing tests in `src/input_mode.rs`

**Files:**
- Modify: `src/input_mode.rs`

- [ ] **Step 1: Append the test module to `src/input_mode.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::NormalizedMotion;

    fn motion(tx: f32, ty: f32, tz: f32, rx: f32, ry: f32, rz: f32) -> NormalizedMotion {
        NormalizedMotion { translation: [tx, ty, tz], rotation: [rx, ry, rz] }
    }

    #[test]
    fn object_manipulation_scales_translation() {
        let mode = InputMode::ObjectManipulation { translation_scale: 2.0, rotation_scale: 1.0 };
        let scaled = process(&motion(0.5, 1.0, -0.5, 0.0, 0.0, 0.0), &mode);
        assert_eq!(scaled.translation, [1.0, 2.0, -1.0]);
        assert_eq!(scaled.rotation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn object_manipulation_scales_rotation() {
        let mode = InputMode::ObjectManipulation { translation_scale: 1.0, rotation_scale: 3.0 };
        let scaled = process(&motion(0.0, 0.0, 0.0, 1.0, 0.5, -0.25), &mode);
        assert_eq!(scaled.translation, [0.0, 0.0, 0.0]);
        assert_eq!(scaled.rotation, [3.0, 1.5, -0.75]);
    }

    #[test]
    fn camera_control_scales_translation() {
        let mode = InputMode::CameraControl { translation_scale: 4.0, rotation_scale: 1.0 };
        let scaled = process(&motion(0.25, -1.0, 0.0, 0.0, 0.0, 0.0), &mode);
        assert_eq!(scaled.translation, [1.0, -4.0, 0.0]);
        assert_eq!(scaled.rotation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn camera_control_scales_rotation() {
        let mode = InputMode::CameraControl { translation_scale: 1.0, rotation_scale: 2.0 };
        let scaled = process(&motion(0.0, 0.0, 0.0, 0.5, -1.0, 0.0), &mode);
        assert_eq!(scaled.translation, [0.0, 0.0, 0.0]);
        assert_eq!(scaled.rotation, [1.0, -2.0, 0.0]);
    }

    #[test]
    fn zero_input_gives_zero_output() {
        let mode = InputMode::object_manipulation_default();
        let scaled = process(&motion(0.0, 0.0, 0.0, 0.0, 0.0, 0.0), &mode);
        assert_eq!(scaled.translation, [0.0, 0.0, 0.0]);
        assert_eq!(scaled.rotation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn object_manipulation_default_has_expected_scales() {
        let InputMode::ObjectManipulation { translation_scale, rotation_scale } =
            InputMode::object_manipulation_default()
        else {
            panic!("wrong variant");
        };
        assert_eq!(translation_scale, 3.0);
        assert_eq!(rotation_scale, 2.0);
    }

    #[test]
    fn camera_control_default_has_expected_scales() {
        let InputMode::CameraControl { translation_scale, rotation_scale } =
            InputMode::camera_control_default()
        else {
            panic!("wrong variant");
        };
        assert_eq!(translation_scale, 5.0);
        assert_eq!(rotation_scale, 1.5);
    }
}
```

- [ ] **Step 2: Run the tests and verify they all fail**

```bash
cargo test
```

Expected: 7 tests, all FAILED with panics at `not yet implemented`.

---

### Task 3: Implement `process()` and presets; verify tests pass

**Files:**
- Modify: `src/input_mode.rs`

- [ ] **Step 1: Replace the three `todo!()` stubs with real implementations**

Replace the `object_manipulation_default` body:
```rust
pub fn object_manipulation_default() -> Self {
    InputMode::ObjectManipulation {
        translation_scale: 3.0,
        rotation_scale: 2.0,
    }
}
```

Replace the `camera_control_default` body:
```rust
pub fn camera_control_default() -> Self {
    InputMode::CameraControl {
        translation_scale: 5.0,
        rotation_scale: 1.5,
    }
}
```

Replace the `process` body:
```rust
pub fn process(input: &crate::NormalizedMotion, mode: &InputMode) -> ScaledMotion {
    let (tx_scale, rx_scale) = match mode {
        InputMode::ObjectManipulation { translation_scale, rotation_scale }
        | InputMode::CameraControl { translation_scale, rotation_scale } => {
            (*translation_scale, *rotation_scale)
        }
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

- [ ] **Step 2: Run the tests and verify all 7 pass**

```bash
cargo test
```

Expected output:
```
test input_mode::tests::camera_control_default_has_expected_scales ... ok
test input_mode::tests::camera_control_scales_rotation ... ok
test input_mode::tests::camera_control_scales_translation ... ok
test input_mode::tests::object_manipulation_default_has_expected_scales ... ok
test input_mode::tests::object_manipulation_scales_rotation ... ok
test input_mode::tests::object_manipulation_scales_translation ... ok
test input_mode::tests::zero_input_gives_zero_output ... ok

test result: ok. 7 passed; 0 failed
```

- [ ] **Step 3: Commit**

```bash
git add src/input_mode.rs src/lib.rs
git commit -m "feat(input_mode): add InputMode, ScaledMotion, and process()"
```

---

### Task 4: Update `examples/cube.rs`

**Files:**
- Modify: `examples/cube.rs`

- [ ] **Step 1: Update imports (lines 1–2)**

Replace:
```rust
use spaceball_rs::{DeviceEvent, SixDofDevice, first, probe};
use std::sync::{Arc, Mutex};
```
With:
```rust
use spaceball_rs::{DeviceEvent, InputMode, SixDofDevice, first, probe, process};
use std::sync::{Arc, Mutex};
use std::time::Instant;
```

- [ ] **Step 2: Replace the background thread (lines 29–51)**

Replace the entire `std::thread::spawn(move || { ... });` block with:

```rust
std::thread::spawn(move || {
    let mode = InputMode::object_manipulation_default();
    let mut last = Instant::now();
    for event in device.events() {
        match event {
            Ok(DeviceEvent::Motion(m)) => {
                let dt = last.elapsed().as_secs_f32();
                last = Instant::now();
                let scaled = process(&m, &mode);
                let mut p = pose_bg.lock().unwrap();
                p.tx += scaled.translation[0] * dt;
                p.ty += scaled.translation[1] * dt;
                p.tz += scaled.translation[2] * dt;
                p.rx += scaled.rotation[0] * dt;
                p.ry += scaled.rotation[1] * dt;
                p.rz += scaled.rotation[2] * dt;
            }
            Ok(DeviceEvent::Button(k)) if k.pressed(0) => {
                *pose_bg.lock().unwrap() = Pose {
                    rx: 25_f32.to_radians(),
                    ry: 35_f32.to_radians(),
                    ..Default::default()
                };
            }
            _ => {}
        }
    }
});
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check --examples
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add examples/cube.rs
git commit -m "feat(cube): use InputMode::object_manipulation_default() and process() with dt"
```

---

### Task 5: Update `examples/asteroids3d.rs`

**Files:**
- Modify: `examples/asteroids3d.rs`

- [ ] **Step 1: Update the spaceball_rs import (line 83)**

Replace:
```rust
use spaceball_rs::{DeviceEvent, first, probe};
```
With:
```rust
use spaceball_rs::{DeviceEvent, InputMode, first, probe, process};
```

- [ ] **Step 2: Add Instant import after line 78**

After `use std::sync::{Arc, Mutex};`, add:
```rust
use std::time::Instant;
```

- [ ] **Step 3: Delete the T_SCALE and R_SCALE constants (lines 85–88)**

Remove these four lines entirely:
```rust
/// Normalized motion values are in [-1, 1] per second at full deflection.
const T_SCALE: f32 = 1.0; // world units per normalized unit
//const R_SCALE: f32 = std::f32::consts::PI; // radians per normalized unit
const R_SCALE: f32 = 0.1;
```

- [ ] **Step 4: Replace the background thread's motion handler**

Inside the `Ok(mut device) =>` arm (around line 250), replace:
```rust
std::thread::spawn(move || {
    let mut prev_fire = false;
    for event in device.events() {
        match event {
            Ok(DeviceEvent::Motion(m)) => {
                let mut s = state_bg.lock().unwrap();
                // Move along the camera's own local axes.
                // translation[2] is negated so that pushing the ball moves you
                // forward (toward asteroids) and pulling backs away.
                let world_move = s.orientation.mul_vec3(Vec3::new(
                    m.translation[0] * T_SCALE,
                    m.translation[1] * T_SCALE,
                    -m.translation[2] * T_SCALE,
                ));
                s.position += world_move;
                // Rotate in the camera's local frame (intrinsic
                // yaw → pitch → roll keeps the horizon intuitive).
                let delta = Quat::from_euler(
                    EulerRot::YXZ,
                    m.rotation[1] * R_SCALE,
                    m.rotation[0] * R_SCALE,
                    m.rotation[2] * R_SCALE,
                );
                s.orientation = (s.orientation * delta).normalize();
            }
```
With:
```rust
std::thread::spawn(move || {
    let mode = InputMode::camera_control_default();
    let mut last = Instant::now();
    let mut prev_fire = false;
    for event in device.events() {
        match event {
            Ok(DeviceEvent::Motion(m)) => {
                let dt = last.elapsed().as_secs_f32();
                last = Instant::now();
                let scaled = process(&m, &mode);
                let mut s = state_bg.lock().unwrap();
                // Move along the camera's own local axes.
                // translation[2] is negated so that pushing the ball moves you
                // forward (toward asteroids) and pulling backs away.
                let world_move = s.orientation.mul_vec3(Vec3::new(
                    scaled.translation[0] * dt,
                    scaled.translation[1] * dt,
                    -scaled.translation[2] * dt,
                ));
                s.position += world_move;
                // Rotate in the camera's local frame (intrinsic
                // yaw → pitch → roll keeps the horizon intuitive).
                let delta = Quat::from_euler(
                    EulerRot::YXZ,
                    scaled.rotation[1] * dt,
                    scaled.rotation[0] * dt,
                    scaled.rotation[2] * dt,
                );
                s.orientation = (s.orientation * delta).normalize();
            }
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo check --examples
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add examples/asteroids3d.rs
git commit -m "feat(asteroids3d): use InputMode::camera_control_default() and process() with dt"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run the full test suite**

```bash
cargo test
```

Expected: 7 tests pass, 0 fail.

- [ ] **Step 2: Verify all examples compile**

```bash
cargo check --examples
```

Expected: no errors.

- [ ] **Step 3: Build all examples**

```bash
cargo build --examples
```

Expected: clean build, no errors.

---

## Notes for implementer

**Scale tuning:** The `camera_control_default()` values (`translation_scale: 5.0`, `rotation_scale: 1.5`) are designed for per-second semantics with `dt`. The old `asteroids3d.rs` used `T_SCALE = 1.0` and `R_SCALE = 0.1` per-event, which is a different unit. Feel will differ — if motion is too fast or slow, adjust the defaults in `InputMode::camera_control_default()`. Similarly for cube.rs (old `3.0 / E` per-event → new `3.0` and `2.0` per-second).

**First-event dt:** On the first motion event, `dt` will be the time since the thread started, which may be large (seconds). This causes a single oversized jump. It's acceptable for Phase 1; Phase 2 can clamp `dt` to a max (e.g., `dt.min(0.1)`).
