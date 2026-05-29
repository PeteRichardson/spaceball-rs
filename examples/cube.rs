use spaceball_rs::{DeviceEvent, InputMode, SixDofDevice, first, probe, process};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use three_d::*;

#[derive(Clone, Copy, Default)]
struct Pose {
    tx: f32,
    ty: f32,
    tz: f32,
    rx: f32,
    ry: f32,
    rz: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1);
    let mut device: Box<dyn SixDofDevice> = match path {
        Some(ref p) => probe(p)?,
        None => first()?,
    };

    let pose = Arc::new(Mutex::new(Pose {
        rx: 25_f32.to_radians(),
        ry: 35_f32.to_radians(),
        ..Default::default()
    }));
    let pose_bg = Arc::clone(&pose);

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

    let window = Window::new(WindowSettings {
        title: "Spaceball Cube".into(),
        max_size: Some((1280, 720)),
        ..Default::default()
    })?;

    let context = window.gl();

    let mut camera = Camera::new_perspective(
        window.viewport(),
        vec3(0.0, 0.0, 5.0),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        0.1,
        100.0,
    );

    let mut cube = Gm::new(
        Mesh::new(&context, &CpuMesh::cube()),
        PhysicalMaterial::new_opaque(
            &context,
            &CpuMaterial {
                albedo: Srgba::new(180, 100, 60, 255),
                metallic: 0.3,
                roughness: 0.5,
                ..Default::default()
            },
        ),
    );

    let ambient = AmbientLight::new(&context, 0.4, Srgba::WHITE);
    let dir_light = DirectionalLight::new(&context, 1.5, Srgba::WHITE, &vec3(-1.0, -2.0, -1.5));

    let mut gui = GUI::new(&context);

    window.render_loop(move |mut frame_input| {
        camera.set_viewport(frame_input.viewport);

        let p = *pose.lock().unwrap();

        cube.geometry.set_transformation(
            Mat4::from_translation(vec3(p.tx, p.ty, p.tz))
                * Mat4::from_angle_y(Rad(p.ry))
                * Mat4::from_angle_x(Rad(p.rx))
                * Mat4::from_angle_z(Rad(p.rz)),
        );

        gui.update(
            &mut frame_input.events,
            frame_input.accumulated_time,
            frame_input.viewport,
            frame_input.device_pixel_ratio,
            |ctx| {
                egui::SidePanel::left("state_panel").show(ctx, |ui| {
                    ui.heading("6DOF Device");
                    ui.separator();

                    ui.label("Translation");
                    ui.monospace(format!("x  {:+.3}\ny  {:+.3}\nz  {:+.3}", p.tx, p.ty, p.tz));

                    ui.separator();

                    ui.label("Rotation (deg)");
                    ui.monospace(format!(
                        "rx {:+.1}\nry {:+.1}\nrz {:+.1}",
                        p.rx.to_degrees(),
                        p.ry.to_degrees(),
                        p.rz.to_degrees()
                    ));

                    ui.separator();
                    ui.label("Button 1/A: reset");
                });
            },
        );

        frame_input
            .screen()
            .clear(ClearState::color_and_depth(0.08, 0.08, 0.12, 1.0, 1.0))
            .render(
                &camera,
                [&cube],
                &[&ambient as &dyn Light, &dir_light as &dyn Light],
            )
            .write(|| gui.render())
            .unwrap();

        FrameOutput::default()
    });

    Ok(())
}
