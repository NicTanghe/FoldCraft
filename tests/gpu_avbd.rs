//! Opt-in hardware checks. Ordinary cargo test never initializes an adapter.
#![cfg(feature = "desktop")]
use marvelous_cloth::{
    gpu_avbd::{ContactMethod, ProjectAvbd, request_device},
    model::Scene,
    project::Project,
};
use std::sync::Arc;

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn resident_free_fall_pins_sphere_and_transactional_failure() {
    check_resident_free_fall_pins_sphere_and_transactional_failure(ContactMethod::Ipc);
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn al_resident_free_fall_pins_sphere_and_transactional_failure() {
    check_resident_free_fall_pins_sphere_and_transactional_failure(ContactMethod::BarrierAl);
}

fn check_resident_free_fall_pins_sphere_and_transactional_failure(method: ContactMethod) {
    let (device, queue) = request_device().expect("hardware compute adapter");
    let mut solver = ProjectAvbd::new(device, queue);
    solver.set_contact_method(method);
    let mut p = Project {
        sphere: None,
        ..Default::default()
    };
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
    let accepted = state.clone();
    state.x[1] = state.x[0];
    let corrupt = state.clone();
    assert!(solver.step(&p, &mesh, &mut state).is_err());
    assert_eq!(state.x, corrupt.x);
    assert_eq!(state.v, corrupt.v);
    assert_eq!(state.time, corrupt.time);
    state = accepted;
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
    let accepted = impact.clone();
    p.sphere.as_mut().unwrap().radius = 2.;
    assert!(solver.step(&p, &mesh, &mut impact).is_err());
    assert_eq!(impact.x, accepted.x);
    assert_eq!(impact.v, accepted.v);
    assert_eq!(impact.time, accepted.time);
    p.sphere.as_mut().unwrap().radius = 0.35;
    solver.step(&p, &mesh, &mut impact).unwrap();
}

mod common;
#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn resident_vf_reciprocal_vf_self_contact_and_translation() {
    check_resident_vf_reciprocal_vf_self_contact_and_translation(ContactMethod::Ipc);
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn al_resident_vf_reciprocal_vf_self_contact_and_translation() {
    check_resident_vf_reciprocal_vf_self_contact_and_translation(ContactMethod::BarrierAl);
}

fn check_resident_vf_reciprocal_vf_self_contact_and_translation(method: ContactMethod) {
    use glam::DVec3;
    use marvelous_cloth::{
        garment::GarmentTemplate,
        gpu_avbd::GpuAvbd,
        vbd::{VbdSettings, VbdSolver},
    };
    let (device, queue) = request_device().unwrap();
    let mut gpu = GpuAvbd::new(device, queue);
    gpu.contact_method = method;
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
            let mut matched = 0;
            for _ in 0..12 {
                let m = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
                assert!(m.contact_terms > 0);
                if method == ContactMethod::BarrierAl {
                    let al = m.barrier_al.as_ref().expect("AL diagnostics");
                    assert!(al.primal_m.is_finite() && al.original_force_max_n.is_finite());
                    matched += al.matched_pairs;
                }
                assert!(m.minimum_distance_m > 0.);
                solver
                    .world
                    .validate_start(&solver.world.positions(&state.x))
                    .unwrap();
            }
            if method == ContactMethod::BarrierAl {
                assert!(matched > 0, "active contacts must retain identities");
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

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn barrier_al_small_matched_cost_probe() {
    use marvelous_cloth::{
        gpu_avbd::GpuAvbd,
        vbd::{VbdSettings, VbdSolver},
    };
    let (device, queue) = request_device().unwrap();
    let (garment, floor) = common::resting_patch(glam::DVec3::ZERO);
    let settings = VbdSettings {
        iterations: 32,
        dt: 1. / 60.,
        ..Default::default()
    };
    let mut rows = Vec::new();
    let mut ipc_reference = Vec::<marvelous_cloth::model::State>::new();
    let mut al_reference = Vec::<marvelous_cloth::model::State>::new();
    for (name, method, balanced, skip) in [
        ("ipc_baseline", ContactMethod::Ipc, true, false),
        ("ipc", ContactMethod::Ipc, true, true),
        (
            "al_balanced_baseline",
            ContactMethod::BarrierAl,
            true,
            false,
        ),
        ("al_fixed", ContactMethod::BarrierAl, false, true),
        ("al_balanced", ContactMethod::BarrierAl, true, true),
    ] {
        let mut gpu = GpuAvbd::new(device.clone(), queue.clone());
        gpu.contact_method = method;
        gpu.balance_augmentation = balanced;
        gpu.skip_accepted_reductions = skip;
        gpu.fuse_small_line_search = skip;
        let mut state = garment.initial_state.clone();
        let mut solver = VbdSolver::new(garment.clone(), Some(&floor), &state).unwrap();
        for step in 0..8 {
            let metrics = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
            solver
                .world
                .validate_start(&solver.world.positions(&state.x))
                .unwrap();
            if name == "ipc_baseline" {
                ipc_reference.push(state.clone());
            }
            if name == "al_balanced_baseline" {
                al_reference.push(state.clone());
            }
            if name == "ipc" || name == "al_balanced" {
                let reference = if name == "ipc" {
                    &ipc_reference[step]
                } else {
                    &al_reference[step]
                };
                for (a, b) in state.x.iter().zip(&reference.x) {
                    assert!(
                        a.distance(*b) < 1e-7,
                        "fused line search changed accepted trajectory"
                    );
                }
                for (a, b) in state.v.iter().zip(&reference.v) {
                    assert!(a.distance(*b) < 1e-5, "fused line search changed velocity");
                }
            }
            rows.push(serde_json::json!({
                "backend":name,"step":step,"step_ms":metrics.step_ms,
                "minimum_m":metrics.minimum_distance_m,
                "extension_max_percent":metrics.extension_max_percent,
                "speed_max_m_s":state.v.iter().map(|v|v.length()).fold(0.,f64::max),
                "al":metrics.barrier_al,
            }));
        }
    }
    println!("BAL_COST_JSON={}", serde_json::to_string(&rows).unwrap());
}
