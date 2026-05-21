//! asteroids3d
//!
//! Usage:  cargo run --example asteroids3d [/dev/cu.usbserial-...]
//!
//! ── Stage 1 ─────────────────────────────────────────────────────────────────
//! Window + static 3-D scene.
//!
//! A perspective camera looks out over a field of rocky spheres — stand-ins for
//! asteroids.  No Spaceball input, no physics.  This stage establishes the
//! rendering baseline before anything else is layered on.
//! ────────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use rand::{rngs::SmallRng, Rng, SeedableRng};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Asteroids 3D".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Camera ───────────────────────────────────────────────────────────────
    // Fixed for now; looking into the field along -Z.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 0.0).looking_at(Vec3::new(0.0, 0.0, -30.0), Vec3::Y),
    ));

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
