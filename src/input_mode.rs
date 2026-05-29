/// Semantic interpretation of 6DOF motion and sensitivity tuning.
#[derive(Debug, Clone, PartialEq)]
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
        // exact: integer-multiple inputs, no intermediate rounding
        let mode = InputMode::ObjectManipulation { translation_scale: 1.0, rotation_scale: 1.0 };
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
