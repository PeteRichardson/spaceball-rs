//! asteroids3d
//!
//! Usage:  cargo run --example asteroids3d [/dev/cu.usbserial-...]
//!
//! ── Stage 1 ─────────────────────────────────────────────────────────────────
//! Window + static 3-D scene.  A perspective camera and 20 rocky spheres.
//!
//! ── Stage 2 ─────────────────────────────────────────────────────────────────
//! POV camera driven by Spaceball 6-DOF input.
//!
//! The camera is the player: ball translation events move it along its own
//! local axes; ball rotation events rotate it in its own local frame (intrinsic
//! yaw-pitch-roll).  Pick button resets to the origin.
//!
//! Spaceball is optional — the scene renders with a static camera if no device
//! is available.
//! ────────────────────────────────────────────────────────────────────────────

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use spaceball_rs::{Packet, Spaceball};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

/// Raw Spaceball values reach ±~16 000 at full deflection.
const T_SCALE: f32 = 3.0 / 16_000.0; // world units per raw unit
const R_SCALE: f32 = std::f32::consts::PI / 16_000.0; // radians per raw unit

// ── Shared player state ──────────────────────────────────────────────────────

struct PlayerState {
    position: Vec3,
    orientation: Quat,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            position: Vec3::ZERO,
            orientation: Quat::IDENTITY,
        }
    }
}

#[derive(Resource)]
struct Player(Arc<Mutex<PlayerState>>);

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    let port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    let player_state = Arc::new(Mutex::new(PlayerState::default()));

    match Spaceball::open(&port) {
        Ok(mut sm) => {
            let state_bg = Arc::clone(&player_state);
            std::thread::spawn(move || {
                for packet in sm.packets() {
                    match packet {
                        Ok(Packet::Ball(b)) => {
                            let [tx, ty, tz] = b.translation;
                            let [rx, ry, rz] = b.rotation;
                            let mut s = state_bg.lock().unwrap();

                            // Move along the camera's own local axes.
                            // tz is negated so that pushing the ball moves you
                            // forward (toward asteroids) and pulling backs away.
                            let world_move = s.orientation.mul_vec3(Vec3::new(
                                tx as f32 * T_SCALE,
                                ty as f32 * T_SCALE,
                                -(tz as f32) * T_SCALE,
                            ));
                            s.position += world_move;

                            // Rotate in the camera's local frame (intrinsic
                            // yaw → pitch → roll keeps the horizon intuitive).
                            let delta = Quat::from_euler(
                                EulerRot::YXZ,
                                ry as f32 * R_SCALE,
                                rx as f32 * R_SCALE,
                                rz as f32 * R_SCALE,
                            );
                            s.orientation = (s.orientation * delta).normalize();
                        }
                        Ok(Packet::Key(k)) => {
                            if k.pick {
                                *state_bg.lock().unwrap() = PlayerState::default();
                            }
                        }
                        _ => {}
                    }
                }
            });
            eprintln!("Spaceball connected on {port}");
        }
        Err(e) => {
            eprintln!("Spaceball not available ({e}); camera is static");
        }
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Asteroids 3D".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Player(player_state))
        .add_systems(Startup, setup)
        .add_systems(Update, update_camera)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Camera ───────────────────────────────────────────────────────────────
    // Transform is overwritten every frame by update_camera; spawn at origin
    // facing -Z (Bevy's default camera forward direction).
    commands.spawn((Camera3d::default(), Transform::default()));

    // ── Lights ───────────────────────────────────────────────────────────────
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 120.0,
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.5, 0.0)),
    ));

    // ── Asteroid placeholders ────────────────────────────────────────────────
    // Spheres scattered in a shell 20–80 units in front of the camera.
    let sphere_mesh = meshes.add(Sphere::new(1.0));
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.50, 0.45, 0.40),
        perceptual_roughness: 0.9,
        ..default()
    });

    let mut rng = SmallRng::seed_from_u64(42);
    for _ in 0..20 {
        // Pick a random direction, rejecting near-zero vectors.
        let dir = loop {
            let v = Vec3::new(
                rng.gen_range(-1.0_f32..1.0),
                rng.gen_range(-1.0_f32..1.0),
                rng.gen_range(-1.0_f32..1.0),
            );
            if v.length_squared() > 1e-4 {
                break v.normalize();
            }
        };

        let dist = rng.gen_range(20.0_f32..80.0);
        let scale = rng.gen_range(0.5_f32..3.0);

        commands.spawn((
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(rock_mat.clone()),
            Transform::from_translation(dir * dist).with_scale(Vec3::splat(scale)),
        ));
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn update_camera(player: Res<Player>, mut query: Query<&mut Transform, With<Camera3d>>) {
    let state = player.0.lock().unwrap();
    if let Ok(mut transform) = query.get_single_mut() {
        transform.translation = state.position;
        transform.rotation = state.orientation;
    }
}
