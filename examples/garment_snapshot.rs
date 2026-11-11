//! One-frame diagnostic render of a saved fitting checkpoint; GPU opt-in.
#![recursion_limit = "256"]
#[cfg(not(feature = "desktop"))]
fn main() {
    eprintln!("garment_snapshot requires the desktop feature and a hardware GPU");
}
#[cfg(feature = "desktop")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use marvelous_cloth::{
        fitting::FittingProject,
        render::{Camera, DrawModel, RenderState, Viewport},
    };
    use std::{path::PathBuf, sync::Arc};
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && args.len() != 4 {
        return Err("usage: garment_snapshot PROJECT OUTPUT.png [YAW PITCH]".into());
    }
    let output = PathBuf::from(&args[1]);
    if output.exists() {
        return Err("refusing to overwrite an existing snapshot".into());
    }
    let yaw = if args.len() == 4 {
        args[2].to_string_lossy().parse::<f32>()?
    } else {
        0.35
    };
    let pitch = if args.len() == 4 {
        args[3].to_string_lossy().parse::<f32>()?
    } else {
        0.12
    };
    let scene = FittingProject::load(&PathBuf::from(&args[0]))?.compile()?;
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err("hardware GPU required".into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let mut viewport = Viewport::new(RenderState {
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            target_format: wgpu::TextureFormat::Rgba8Unorm,
        });
        let camera = Camera {
            yaw,
            pitch,
            distance: 2.65,
            target: glam::Vec3::new(0., 0.91, 0.),
            ..Default::default()
        };
        viewport.draw_scene(
            [1000, 1000],
            &camera,
            DrawModel {
                optics: None,
                marked_faces: None,
                mesh: &scene.garment.mesh,
                state: &scene.state,
                materials: &scene.garment.materials,
                sphere: None,
                avatar: Some(&scene.collider.surface),
                collider: Some(scene.collider.as_ref()),
                seams: &scene.garment.seams,
            },
            false,
            false,
            false,
        );
        viewport.capture(&output)?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
