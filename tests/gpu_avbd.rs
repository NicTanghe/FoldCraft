//! Opt-in hardware checks. Ordinary cargo test never initializes an adapter.
#![cfg(feature = "desktop")]
use marvelous_cloth::{
    gpu_avbd::{ProjectAvbd, request_device},
    model::Scene,
    project::Project,
};
use std::sync::Arc;

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn resident_free_fall_pins_sphere_and_transactional_failure() {
    let (device, queue) = request_device().expect("hardware compute adapter");
    let mut solver = ProjectAvbd::new(device, queue);
    let mut p = Project::default();
    p.sphere = None;
    p.pattern.nx = 2;
    p.pattern.ny = 2;
    let (mesh, mut state) = p.build().unwrap();
    let mesh = Arc::new(mesh);
    let initial = state.clone();
    let dt = p.solver.dt;
    let damp = 1. + dt * p.solver.damping;
    for step in 1..=3 {
        let old = state.clone();
        let m = solver.step(&p, &mesh, &mut state).unwrap();
        assert_eq!(m.time_s, step as f64 * dt);
        for i in 0..state.x.len() {
            let expected = old.x[i] + (old.v[i] * dt + p.solver.gravity * dt * dt) / damp;
            assert!(
                state.x[i].distance(expected) < 2e-5,
                "free fall {step}/{i}: {:?} vs {:?}",
                state.x[i],
                expected
            );
        }
    }
    let accepted = state.clone();
    let mut bad = p.clone();
    bad.solver.dt = f64::NAN;
    assert!(solver.step(&bad, &mesh, &mut state).is_err());
    assert_eq!(state.x, accepted.x);
    assert_eq!(state.v, accepted.v);
    assert_eq!(state.time, accepted.time);
    solver.step(&p, &mesh, &mut state).unwrap();
    // Replacing the rest mesh must rebuild every dependent resource.
    p.scene = Scene::Hanging;
    let (mesh, mut pinned) = p.build().unwrap();
    let mesh = Arc::new(mesh);
    let original = pinned.clone();
    solver.step(&p, &mesh, &mut pinned).unwrap();
    for (i, pin) in mesh.pins.iter().enumerate() {
        if pin.is_some() {
            assert_eq!(pinned.x[i], original.x[i]);
            assert_eq!(pinned.v[i], glam::DVec3::ZERO);
        }
    }
    assert_ne!(pinned.x, initial.x);
    p.scene = Scene::Sphere;
    p.sphere = Some(Default::default());
    let (mesh, mut impact) = p.build().unwrap();
    let mesh = Arc::new(mesh);
    for x in &mut impact.x {
        x.y = 0.352;
    }
    for v in &mut impact.v {
        v.y = -40.;
    }
    for _ in 0..3 {
        solver.step(&p, &mesh, &mut impact).unwrap();
        for x in &impact.x {
            assert!(x.length() > 0.3505);
        }
    }
}

mod common;
#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn resident_vf_reciprocal_vf_self_contact_and_translation() {
    use glam::DVec3;
    use marvelous_cloth::{
        garment::GarmentTemplate,
        gpu_avbd::GpuAvbd,
        vbd::{VbdSettings, VbdSolver},
    };
    let (device, queue) = request_device().unwrap();
    let gpu = GpuAvbd::new(device, queue);
    let settings = VbdSettings {
        dt: 1. / 60.,
        iterations: 8,
        ..Default::default()
    };
    for small_floor in [false, true] {
        let mut reference = None::<Vec<DVec3>>;
        for offset in [DVec3::ZERO, DVec3::new(100., 10., -100.)] {
            let (g, mut floor) = common::resting_patch(offset);
            if small_floor {
                floor.vertices = vec![
                    offset + DVec3::new(-0.01, 0., -0.01),
                    offset + DVec3::new(0.01, 0., -0.01),
                    offset + DVec3::new(0., 0., 0.01),
                ];
            }
            let mut state = g.initial_state.clone();
            for v in &mut state.v {
                v.y = -2.;
            }
            let mut solver = VbdSolver::new(g, Some(&floor), &state).unwrap();
            for _ in 0..12 {
                let m = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
                assert!(m.contact_terms > 0);
                assert!(m.minimum_distance_m > 0.);
                solver
                    .world
                    .validate_start(&solver.world.positions(&state.x))
                    .unwrap();
            }
            let local: Vec<_> = state.x.iter().map(|p| *p - offset).collect();
            if let Some(reference) = &reference {
                for (a, b) in local.iter().zip(reference) {
                    assert!(
                        a.distance(*b) < 5e-6,
                        "world translation changed contact: {a:?} / {b:?}"
                    );
                }
            } else {
                reference = Some(local);
            }
        }
    }
    let mut t = GarmentTemplate::two_panel_demo();
    t.seams.clear();
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pins.clear();
        p.pattern = marvelous_cloth::model::Pattern {
            width: 0.12,
            height: 0.12,
            nx: 2,
            ny: 2,
        };
        p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
        p.placement.translation = DVec3::new(-0.06, 0.003 + 0.003 * i as f64, 0.06);
    }
    let g = Arc::new(t.compile().unwrap());
    let mut state = g.initial_state.clone();
    let split = g.panels[1].vertices.start;
    for (i, v) in state.v.iter_mut().enumerate() {
        v.y = if i < split { 1. } else { -1. };
    }
    let mut solver = VbdSolver::new(g.clone(), None, &state).unwrap();
    let s = VbdSettings {
        gravity: DVec3::ZERO,
        ..settings
    };
    for _ in 0..12 {
        gpu.step(&mut solver, &mut state, &s, None).unwrap();
        solver.world.validate_start(&state.x).unwrap();
        let bottom = state.x[..split]
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let top = state.x[split..]
            .iter()
            .map(|p| p.y)
            .fold(f64::INFINITY, f64::min);
        assert!(top > bottom, "cloth layers crossed: {top} vs {bottom}");
    }
}
