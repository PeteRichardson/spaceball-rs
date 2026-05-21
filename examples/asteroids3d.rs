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
//!
//! ── Stage 3 ─────────────────────────────────────────────────────────────────
//! Asteroid field.
//!
//! Placeholder spheres become proper Asteroid entities carrying a radius field
//! (needed for collision in Stage 6).  Spawning is extracted into
//! spawn_asteroid_field so later stages can refill the field as the player
//! moves.  A HUD overlay shows the live asteroid count.
//!
//! ── Stage 4 ─────────────────────────────────────────────────────────────────
//! Asteroid visual character.
//!
//! Each asteroid gets a unique procedurally-generated lumpy mesh: a sphere with
//! vertices randomly perturbed along their radial direction.  Flat normals give
//! the surface a faceted, rock-like appearance.
//!
//! ── Stage 5 ─────────────────────────────────────────────────────────────────
//! Asteroid drift.
//!
//! Each asteroid moves with a constant linear velocity and spins on a random
//! axis.  When an asteroid drifts farther than ASTEROID_RESPAWN_DIST from the
//! player it is despawned and a fresh one is spawned nearby, keeping the field
//! always populated around the player.
//!
//! ── Stage 6 ─────────────────────────────────────────────────────────────────
//! Asteroid collisions.
//!
//! When two asteroid spheres overlap, both are destroyed and each independently
//! splits into two fragments that fly apart along the collision normal.
//! Fragments below ASTEROID_MIN_RADIUS simply vanish.
//! A refill_asteroids system tops up the field after any net loss.
//! ────────────────────────────────────────────────────────────────────────────

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use spaceball_rs::{Packet, Spaceball};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

/// Raw Spaceball values reach ±~16 000 at full deflection.
const T_SCALE: f32 = 3.0 / 16_000.0; // world units per raw unit
const R_SCALE: f32 = std::f32::consts::PI / 16_000.0; // radians per raw unit

/// Number of asteroids kept alive in the field at all times.
const ASTEROID_COUNT: usize = 40;
/// Asteroids are spawned in a shell at this distance range from the player.
const ASTEROID_MIN_DIST: f32 = 20.0;
const ASTEROID_MAX_DIST: f32 = 80.0;
/// Number of background stars.
const STAR_COUNT: usize = 600;
/// Asteroids beyond this distance from the player are despawned and respawned.
const ASTEROID_RESPAWN_DIST: f32 = 160.0;
/// Asteroid radius below which a collision produces no fragments.
const ASTEROID_MIN_RADIUS: f32 = 0.3;
/// Speed added to each fragment along the collision normal on breakup (u/s).
const FRAGMENT_SPEED: f32 = 3.0;

// ── Components ───────────────────────────────────────────────────────────────

/// Marker + data for asteroid entities.
#[derive(Component)]
#[allow(dead_code)]
struct Asteroid {
    /// World-space collision radius (= Transform scale, since base mesh r = 1).
    radius: f32,
    /// Constant linear velocity in world space (units/sec).
    velocity: Vec3,
    /// Angular velocity as an axis-angle vector in world space (radians/sec).
    angular_velocity: Vec3,
}

/// Marker for the HUD text node that displays the asteroid count.
#[derive(Component)]
struct AsteroidCountText;

/// Material handle kept alive so despawned asteroids can be respawned cheaply.
#[derive(Resource)]
struct AsteroidAssets {
    mat: Handle<StandardMaterial>,
}

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
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Player(player_state))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                update_camera,
                update_hud,
                drift_asteroids,
                asteroid_collisions,
                refill_asteroids,
            ),
        )
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

    // ── Star field ─────────────────────────────────────────────────────────────
    // Stars are tiny emissive spheres placed very far away so they appear fixed.
    let star_mesh = meshes.add(Sphere::new(1.0));
    let star_mat = materials.add(StandardMaterial {
        emissive: LinearRgba::new(4.0, 4.2, 5.0, 1.0), // cool blue-white glow
        ..default()
    });
    let mut star_rng = SmallRng::seed_from_u64(7);
    for _ in 0..STAR_COUNT {
        let dir = loop {
            let v = Vec3::new(
                star_rng.gen_range(-1.0_f32..1.0),
                star_rng.gen_range(-1.0_f32..1.0),
                star_rng.gen_range(-1.0_f32..1.0),
            );
            if v.length_squared() > 1e-4 {
                break v.normalize();
            }
        };
        let dist = star_rng.gen_range(1_000.0_f32..3_000.0_f32);
        let size = star_rng.gen_range(0.4_f32..1.2_f32);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::from_translation(dir * dist).with_scale(Vec3::splat(size)),
        ));
    }

    // ── Asteroid field ────────────────────────────────────────────────────────
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.50, 0.45, 0.40),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.insert_resource(AsteroidAssets {
        mat: rock_mat.clone(),
    });
    let mut rng = SmallRng::seed_from_u64(42);
    spawn_asteroid_field(&mut commands, &mut meshes, &rock_mat, &mut rng);

    // ── HUD ───────────────────────────────────────────────────────────────────
    commands.spawn((
        Text::new(format!("Asteroids: {ASTEROID_COUNT}")),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        AsteroidCountText,
    ));
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn update_camera(player: Res<Player>, mut query: Query<&mut Transform, With<Camera3d>>) {
    let state = player.0.lock().unwrap();
    if let Ok(mut transform) = query.get_single_mut() {
        transform.translation = state.position;
        transform.rotation = state.orientation;
    }
}

fn spawn_asteroid_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mat: &Handle<StandardMaterial>,
    rng: &mut impl Rng,
) {
    for _ in 0..ASTEROID_COUNT {
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
        let dist = rng.gen_range(ASTEROID_MIN_DIST..ASTEROID_MAX_DIST);
        let scale = rng.gen_range(0.5_f32..6.0_f32);
        let rock_mesh = make_rock_mesh(meshes, rng);
        let velocity = Vec3::new(
            rng.gen_range(-1.5_f32..1.5),
            rng.gen_range(-1.5_f32..1.5),
            rng.gen_range(-1.5_f32..1.5),
        );
        let angular_velocity = Vec3::new(
            rng.gen_range(-0.3_f32..0.3),
            rng.gen_range(-0.3_f32..0.3),
            rng.gen_range(-0.3_f32..0.3),
        );

        commands.spawn((
            Mesh3d(rock_mesh),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(dir * dist).with_scale(Vec3::splat(scale)),
            Asteroid {
                radius: scale,
                velocity,
                angular_velocity,
            },
        ));
    }
}

/// Build a unique lumpy rock mesh by randomly perturbing sphere vertices.
///
/// Each vertex is scaled along its radial direction by an independent random
/// factor (0.75–1.25), then flat normals are computed for a faceted look.
fn make_rock_mesh(meshes: &mut Assets<Mesh>, rng: &mut impl Rng) -> Handle<Mesh> {
    let mut mesh: Mesh = Sphere::new(1.0).into();

    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for pos in positions.iter_mut() {
            let factor = rng.gen_range(0.85_f32..1.15_f32);
            pos[0] *= factor;
            pos[1] *= factor;
            pos[2] *= factor;
        }
    }

    // Flat normals require each triangle to have its own vertex copies.
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

    meshes.add(mesh)
}

fn update_hud(
    asteroids: Query<(), With<Asteroid>>,
    mut text_query: Query<&mut Text, With<AsteroidCountText>>,
) {
    let count = asteroids.iter().count();
    if let Ok(mut t) = text_query.get_single_mut() {
        *t = Text::new(format!("Asteroids: {count}"));
    }
}

fn drift_asteroids(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AsteroidAssets>,
    player: Res<Player>,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &Asteroid)>,
) {
    let dt = time.delta_secs();
    let player_pos = player.0.lock().unwrap().position;
    let mut rng = rand::thread_rng();

    for (entity, mut transform, asteroid) in &mut query {
        // Translate at constant velocity.
        transform.translation += asteroid.velocity * dt;

        // Spin: intrinsic rotation each frame.
        let spin = Quat::from_scaled_axis(asteroid.angular_velocity * dt);
        transform.rotation = (transform.rotation * spin).normalize();

        // Respawn when too far from the player.
        if (transform.translation - player_pos).length() > ASTEROID_RESPAWN_DIST {
            commands.entity(entity).despawn();

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
            let dist = rng.gen_range(ASTEROID_MIN_DIST..ASTEROID_MAX_DIST);
            let scale = rng.gen_range(0.5_f32..6.0_f32);
            let rock_mesh = make_rock_mesh(&mut meshes, &mut rng);
            let velocity = Vec3::new(
                rng.gen_range(-1.5_f32..1.5),
                rng.gen_range(-1.5_f32..1.5),
                rng.gen_range(-1.5_f32..1.5),
            );
            let angular_velocity = Vec3::new(
                rng.gen_range(-0.3_f32..0.3),
                rng.gen_range(-0.3_f32..0.3),
                rng.gen_range(-0.3_f32..0.3),
            );

            commands.spawn((
                Mesh3d(rock_mesh),
                MeshMaterial3d(assets.mat.clone()),
                Transform::from_translation(player_pos + dir * dist).with_scale(Vec3::splat(scale)),
                Asteroid {
                    radius: scale,
                    velocity,
                    angular_velocity,
                },
            ));
        }
    }
}

fn asteroid_collisions(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AsteroidAssets>,
    query: Query<(Entity, &Transform, &Asteroid)>,
) {
    // Snapshot all data upfront to enable pair iteration without borrow conflicts.
    let asteroids: Vec<(Entity, Vec3, f32, Vec3, Vec3)> = query
        .iter()
        .map(|(e, t, a)| (e, t.translation, a.radius, a.velocity, a.angular_velocity))
        .collect();

    let mut handled: Vec<Entity> = Vec::new();

    for i in 0..asteroids.len() {
        for j in (i + 1)..asteroids.len() {
            let (e_a, pos_a, r_a, vel_a, ang_a) = asteroids[i];
            let (e_b, pos_b, r_b, vel_b, ang_b) = asteroids[j];

            if handled.contains(&e_a) || handled.contains(&e_b) {
                continue;
            }
            if pos_a.distance(pos_b) >= r_a + r_b {
                continue;
            }

            // Both asteroids are destroyed; each independently tries to split.
            commands.entity(e_a).despawn();
            commands.entity(e_b).despawn();
            handled.push(e_a);
            handled.push(e_b);

            // Normal from A toward B — used to orient the fragment spread.
            let delta = pos_b - pos_a;
            let normal = if delta.length_squared() > 1e-8 {
                delta.normalize()
            } else {
                Vec3::X
            };

            let mut rng = rand::thread_rng();

            // Each asteroid produces two fragments if it is large enough.
            for &(pos, r, vel, ang) in &[(pos_a, r_a, vel_a, ang_a), (pos_b, r_b, vel_b, ang_b)] {
                let frag_r = r * 0.55;
                if frag_r < ASTEROID_MIN_RADIUS {
                    continue; // too small — just vanishes
                }
                for &sign in &[-1.0_f32, 1.0_f32] {
                    let rock_mesh = make_rock_mesh(&mut meshes, &mut rng);
                    commands.spawn((
                        Mesh3d(rock_mesh),
                        MeshMaterial3d(assets.mat.clone()),
                        Transform::from_translation(pos + normal * sign * frag_r * 1.5)
                            .with_scale(Vec3::splat(frag_r)),
                        Asteroid {
                            radius: frag_r,
                            velocity: vel + normal * sign * FRAGMENT_SPEED,
                            angular_velocity: ang * 1.5,
                        },
                    ));
                }
            }
        }
    }
}

/// Spawn fresh asteroids near the player whenever collisions reduce the count.
fn refill_asteroids(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AsteroidAssets>,
    player: Res<Player>,
    query: Query<(), With<Asteroid>>,
) {
    let count = query.iter().count();
    if count >= ASTEROID_COUNT {
        return;
    }
    let player_pos = player.0.lock().unwrap().position;
    let mut rng = rand::thread_rng();
    for _ in count..ASTEROID_COUNT {
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
        let dist = rng.gen_range(ASTEROID_MIN_DIST..ASTEROID_MAX_DIST);
        let scale = rng.gen_range(0.5_f32..6.0_f32);
        let rock_mesh = make_rock_mesh(&mut meshes, &mut rng);
        let velocity = Vec3::new(
            rng.gen_range(-1.5_f32..1.5),
            rng.gen_range(-1.5_f32..1.5),
            rng.gen_range(-1.5_f32..1.5),
        );
        let angular_velocity = Vec3::new(
            rng.gen_range(-0.3_f32..0.3),
            rng.gen_range(-0.3_f32..0.3),
            rng.gen_range(-0.3_f32..0.3),
        );
        commands.spawn((
            Mesh3d(rock_mesh),
            MeshMaterial3d(assets.mat.clone()),
            Transform::from_translation(player_pos + dir * dist).with_scale(Vec3::splat(scale)),
            Asteroid {
                radius: scale,
                velocity,
                angular_velocity,
            },
        ));
    }
}
