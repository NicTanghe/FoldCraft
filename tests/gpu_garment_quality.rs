//! Long opt-in quality regression for the reported default T-shirt protrusion.
//! Separate target: the small GPU suite does not launch these garment runs.
#![cfg(feature = "desktop")]
use marvelous_cloth::{
    fitting::FittingProject,
    garment::{diagnostics::extension_percent, presets::GarmentKind},
    gpu_avbd::{ContactMethod, GpuAvbd, request_device},
    vbd::{VbdSettings, VbdSolver},
};

#[test]
#[ignore = "long hardware GPU comparison; coordinate use explicitly"]
fn default_tshirt_matches_cpu_quality_after_200_steps() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()
        .ok();
    let project = FittingProject::basic(
        GarmentKind::Tshirt,
        2,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/makehuman/mannequin.obj"),
    );
    let scene = project.compile().unwrap();
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    let mut control = scene.state.clone();
    let mut cpu = VbdSolver::new(
        scene.garment.clone(),
        Some(&scene.collider.surface),
        &control,
    )
    .unwrap();
    for _ in 0..200 {
        cpu.step(&mut control, &settings, None).unwrap();
    }
    let cpu_gap = scene.garment.max_seam_gap(&control.x);
    let cpu_extension = extension_percent(&scene.garment, &control)[1];
    eprintln!("CPU control: gap={cpu_gap} m, extension={cpu_extension}%");
    let (device, queue) = request_device().unwrap();
    for method in [ContactMethod::Ipc, ContactMethod::BarrierAl] {
        let mut gpu = GpuAvbd::new(device.clone(), queue.clone());
        gpu.contact_method = method;
        let mut state = scene.state.clone();
        let mut solver =
            VbdSolver::new(scene.garment.clone(), Some(&scene.collider.surface), &state).unwrap();
        for step in 0..200 {
            gpu.step(&mut solver, &mut state, &settings, None).unwrap();
            if step % 50 == 49 {
                solver
                    .world
                    .validate_start(&solver.world.positions(&state.x))
                    .unwrap();
                eprintln!("{method:?}: accepted {} steps", step + 1);
            }
        }
        let gap = scene.garment.max_seam_gap(&state.x);
        let extension = extension_percent(&scene.garment, &state)[1];
        let rms = (state
            .x
            .iter()
            .zip(&control.x)
            .map(|(a, b)| a.distance_squared(*b))
            .sum::<f64>()
            / state.x.len() as f64)
            .sqrt();
        eprintln!("{method:?}: gap={gap} m, extension={extension}%, CPU displacement RMS={rms} m");
        assert!(
            gap < 1.5 * cpu_gap,
            "sewing closure regressed from CPU control"
        );
        assert!(
            extension < 1.25 * cpu_extension + 2.,
            "persistent excessive stretch"
        );
        assert!(rms < 0.012, "reported garment-shape distortion returned");
    }
}
