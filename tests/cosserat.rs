use glam::{DQuat, DVec3};
use marvelous_cloth::cosserat::{OrientationMethod, RodMaterial, RodModel, RodSettings};

fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 1.0,
        stretch_shear_rigidity: 10.0,
        bend_twist_rigidity: 0.2,
    }
}
fn chain(pins: &[usize], pinned_frames: &[usize]) -> RodModel {
    RodModel::new(
        vec![DVec3::ZERO, DVec3::X, 2.0 * DVec3::X],
        vec![[0, 1], [1, 2]],
        vec![[0, 1]],
        None,
        pins,
        pinned_frames,
        material(),
    )
    .unwrap()
}
fn settings() -> RodSettings {
    RodSettings {
        dt_s: 0.05,
        iterations: 80,
        gravity_m_s2: DVec3::ZERO,
        drag_per_s: 0.0,
        orientation_method: OrientationMethod::Approximate,
        contact: None,
    }
}
fn negate(q: DQuat) -> DQuat {
    DQuat::from_xyzw(-q.x, -q.y, -q.z, -q.w)
}

#[test]
fn rest_geometry_is_stress_free_and_objective_with_curved_rest_shape() {
    let model = RodModel::new(
        vec![DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0)],
        vec![[0, 1], [1, 2]],
        vec![[0, 1]],
        None,
        &[],
        &[],
        material(),
    )
    .unwrap();
    let state = model.initial_state();
    assert!(model.elastic_energy(&state).unwrap() < 1e-27);
    let mut deformed = state.clone();
    deformed.positions[2] += DVec3::new(0.15, -0.1, 0.2);
    deformed.orientations[1] *= DQuat::from_rotation_x(0.3);
    let energy = model.elastic_energy(&deformed).unwrap();
    assert!(energy > 0.0);
    let rotation = DQuat::from_rotation_y(0.7) * DQuat::from_rotation_z(-0.4);
    let translation = DVec3::new(4.0, -2.0, 1.0);
    for p in &mut deformed.positions {
        *p = rotation * *p + translation;
    }
    for q in &mut deformed.orientations {
        *q = rotation * *q;
    }
    let rotated = model.elastic_energy(&deformed).unwrap();
    assert!((energy - rotated).abs() < 1e-12, "{energy} vs {rotated}");
    for q in &mut deformed.orientations {
        *q = negate(*q);
    }
    assert!((energy - model.elastic_energy(&deformed).unwrap()).abs() < 1e-12);
}

#[test]
fn stretch_energy_matches_analytic_force_by_finite_difference() {
    let model = RodModel::new(
        vec![DVec3::ZERO, DVec3::X],
        vec![[0, 1]],
        vec![],
        None,
        &[0],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    state.positions[1] = DVec3::new(1.2, 0.1, -0.05);
    state.orientations[0] = DQuat::from_rotation_z(0.2);
    let expected =
        material().stretch_shear_rigidity * (state.positions[1] - state.orientations[0] * DVec3::X);
    let epsilon = 1e-6;
    for axis in 0..3 {
        let mut plus = state.clone();
        let mut minus = state.clone();
        plus.positions[1][axis] += epsilon;
        minus.positions[1][axis] -= epsilon;
        let observed = (model.elastic_energy(&plus).unwrap()
            - model.elastic_energy(&minus).unwrap())
            / (2.0 * epsilon);
        assert!(
            (observed - expected[axis]).abs() < 1e-8,
            "{observed} vs {}",
            expected[axis]
        );
    }
}

#[test]
fn axial_implicit_step_matches_closed_form_and_force_balance() {
    let model = RodModel::new(
        vec![DVec3::ZERO, DVec3::X],
        vec![[0, 1]],
        vec![],
        None,
        &[0],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    let mut cfg = settings();
    cfg.dt_s = 0.1;
    cfg.gravity_m_s2 = 3.0 * DVec3::X;
    let result = model.step(&mut state, cfg).unwrap();
    let mass = 0.5;
    let expected = 1.0
        + 3.0 * cfg.dt_s.powi(2)
            / (1.0 + material().stretch_shear_rigidity * cfg.dt_s.powi(2) / mass);
    assert!(
        (state.positions[1].x - expected).abs() < 1e-12,
        "{} vs {expected}",
        state.positions[1].x
    );
    assert_eq!(state.positions[0], DVec3::ZERO);
    assert!((state.velocities[1].x - (expected - 1.0) / cfg.dt_s).abs() < 1e-12);
    assert!(result.position_residual_rms_n.unwrap() < 1e-10);
    assert_eq!(result.time_s, cfg.dt_s);
}

#[test]
fn free_fall_preserves_shape_and_uses_backward_euler_time() {
    let model = chain(&[], &[]);
    let mut state = model.initial_state();
    let mut cfg = settings();
    cfg.gravity_m_s2 = DVec3::new(0.0, -9.81, 0.0);
    for _ in 0..10 {
        model.step(&mut state, cfg).unwrap();
    }
    let displacement = cfg.gravity_m_s2 * cfg.dt_s.powi(2) * 55.0;
    for (i, p) in state.positions.iter().enumerate() {
        assert!((*p - (i as f64 * DVec3::X + displacement)).length() < 1e-9);
    }
    assert!(model.elastic_energy(&state).unwrap() < 1e-18);
    assert!((state.time_s - 10.0 * cfg.dt_s).abs() < 1e-12);
}

#[test]
fn twist_relaxes_and_pinned_orientation_is_preserved_in_both_modes() {
    let model = chain(&[0, 1, 2], &[0]);
    for method in [OrientationMethod::Approximate, OrientationMethod::Refined] {
        let mut state = model.initial_state();
        state.orientations[1] = DQuat::from_rotation_x(0.7);
        let before = model.elastic_energy(&state).unwrap();
        assert!(before > 0.0);
        let pinned = state.orientations[0];
        let mut cfg = settings();
        cfg.orientation_method = method;
        for _ in 0..8 {
            model.step(&mut state, cfg).unwrap();
        }
        let after = model.elastic_energy(&state).unwrap();
        assert!(after < before * 0.01, "{method:?}: {before} -> {after}");
        assert_eq!(state.orientations[0], pinned);
        assert!(
            state
                .orientations
                .iter()
                .all(|q| (q.length_squared() - 1.0).abs() < 1e-12)
        );
    }
}

#[test]
fn invalid_input_and_collapsed_state_are_rejected_transactionally() {
    assert!(
        RodModel::new(
            vec![DVec3::ZERO, DVec3::ZERO],
            vec![[0, 1]],
            vec![],
            None,
            &[],
            &[],
            material()
        )
        .is_err()
    );
    assert!(
        RodModel::new(
            vec![DVec3::ZERO, DVec3::X],
            vec![[0, 1], [1, 0]],
            vec![],
            None,
            &[],
            &[],
            material()
        )
        .is_err()
    );
    assert!(
        RodModel::new(
            vec![DVec3::ZERO, DVec3::X],
            vec![[0, 1]],
            vec![],
            Some(vec![DQuat::from_rotation_z(1.0)]),
            &[],
            &[],
            material()
        )
        .is_err()
    );
    let model = RodModel::new(
        vec![DVec3::ZERO, DVec3::X],
        vec![[0, 1]],
        vec![],
        None,
        &[0, 1],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    state.positions[1] = DVec3::ZERO;
    let before = state.clone();
    assert!(model.step(&mut state, settings()).is_err());
    assert_eq!(state, before);
    let mut invalid = settings();
    invalid.dt_s = f64::NAN;
    assert!(model.step(&mut state, invalid).is_err());
    assert_eq!(state, before);
}

#[test]
fn antipodal_quaternions_give_the_same_physical_step() {
    let model = chain(&[0], &[0]);
    let mut a = model.initial_state();
    a.positions[2] += DVec3::new(0.1, 0.2, -0.1);
    a.orientations[1] = DQuat::from_rotation_x(0.2) * DQuat::from_rotation_z(0.1);
    let mut b = a.clone();
    b.orientations[1] = negate(b.orientations[1]);
    model.step(&mut a, settings()).unwrap();
    model.step(&mut b, settings()).unwrap();
    for (p, q) in a.positions.iter().zip(&b.positions) {
        assert!((*p - *q).length() < 1e-11);
    }
    for (p, q) in a.orientations.iter().zip(&b.orientations) {
        assert!(p.dot(*q).abs() > 1.0 - 1e-11);
    }
    assert!((model.elastic_energy(&a).unwrap() - model.elastic_energy(&b).unwrap()).abs() < 1e-12);
}

#[test]
fn additional_sweeps_reduce_a_pinned_cantilever_positional_residual() {
    let model = chain(&[0], &[0]);
    let initial = model.initial_state();
    let mut coarse = initial.clone();
    let mut fine = initial;
    let mut cfg = settings();
    cfg.gravity_m_s2 = DVec3::new(0.0, -9.81, 0.0);
    cfg.iterations = 1;
    let first = model.step(&mut coarse, cfg).unwrap();
    cfg.iterations = 80;
    let last = model.step(&mut fine, cfg).unwrap();
    assert!(
        last.position_residual_rms_n.unwrap() < first.position_residual_rms_n.unwrap(),
        "{} -> {}",
        first.position_residual_rms_n.unwrap(),
        last.position_residual_rms_n.unwrap()
    );
    assert_eq!(coarse.positions[0], DVec3::ZERO);
    assert_eq!(fine.positions[0], DVec3::ZERO);
}

#[test]
fn rod_barrier_derivatives_match_finite_differences_and_activation() {
    use marvelous_cloth::cosserat::RodContact;
    let barrier = RodContact {
        activation_m: 0.1,
        stiffness_j: 0.005,
        clearance_m: 1e-8,
    };
    for d in [0.005, 0.03, 0.07, 0.099] {
        let h = 1e-7;
        let (energy, gradient, curvature) = barrier.barrier(d).unwrap();
        let plus = barrier.barrier(d + h).unwrap();
        let minus = barrier.barrier(d - h).unwrap();
        assert!(energy > 0.0 && gradient < 0.0 && curvature > 0.0);
        assert!(((plus.0 - minus.0) / (2.0 * h) - gradient).abs() < 1e-7 * gradient.abs().max(1.0));
        assert!(
            ((plus.1 - minus.1) / (2.0 * h) - curvature).abs() < 1e-7 * curvature.abs().max(1.0)
        );
    }
    assert_eq!(barrier.barrier(0.1).unwrap(), (0.0, 0.0, 0.0));
    assert!(barrier.barrier(1e-8).is_err());
}

#[test]
fn nonadjacent_rods_resist_crossing_and_invalid_starts_are_atomic() {
    use marvelous_cloth::cosserat::RodContact;
    let rest = vec![
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::new(0.0, -0.5, 0.04),
        DVec3::new(0.0, 0.5, 0.04),
    ];
    let model = RodModel::new(
        rest,
        vec![[0, 1], [2, 3]],
        vec![],
        None,
        &[0, 1],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    state.velocities[2] = DVec3::new(0.0, 0.0, -1.0);
    state.velocities[3] = state.velocities[2];
    let mut config = settings();
    config.dt_s = 0.1;
    config.iterations = 16;
    config.contact = Some(RodContact {
        activation_m: 0.05,
        stiffness_j: 0.002,
        clearance_m: 1e-6,
    });
    let mut without_contact = state.clone();
    let mut control = config;
    control.contact = None;
    model.step(&mut without_contact, control).unwrap();
    assert!(without_contact.positions[2].z < 0.0 && without_contact.positions[3].z < 0.0);
    let metrics = model.step(&mut state, config).unwrap();
    assert!(metrics.minimum_nonadjacent_distance_m.unwrap() > 1e-6);
    assert!(metrics.contact_energy_j.is_finite() && metrics.contact_energy_j > 0.0);
    // A tilted rod may have an endpoint below the other rod's plane without
    // crossing it. Check separation where the moving segment crosses y=0.
    let a = state.positions[2];
    let b = state.positions[3];
    let at_intersection = a.lerp(b, -a.y / (b.y - a.y));
    assert!(at_intersection.z > 1e-6, "{:?}", state.positions);
    assert!(metrics.bounded_updates > 0);
    let mut invalid = model.initial_state();
    invalid.positions[2].z = 0.0;
    invalid.positions[3].z = 0.0;
    let before = invalid.clone();
    assert!(model.step(&mut invalid, config).is_err());
    assert_eq!(invalid, before);
}

#[test]
fn connected_segments_are_excluded_from_rod_self_contact() {
    use marvelous_cloth::cosserat::RodContact;
    let model = chain(&[0], &[0]);
    let mut state = model.initial_state();
    let mut cfg = settings();
    cfg.contact = Some(RodContact {
        activation_m: 0.05,
        stiffness_j: 0.002,
        clearance_m: 1e-6,
    });
    let metrics = model.step(&mut state, cfg).unwrap();
    assert_eq!(metrics.minimum_nonadjacent_distance_m, None);
    assert_eq!(metrics.contact_energy_j, 0.0);
    assert!(model.elastic_energy(&state).unwrap() < 1e-25);
}

#[test]
fn rod_contact_force_matches_energy_gradient_for_interior_edge_pair() {
    use marvelous_cloth::cosserat::RodContact;
    let positions = vec![
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::new(0.0, -0.5, 0.02),
        DVec3::new(0.0, 0.5, 0.02),
    ];
    let model = RodModel::new(
        positions,
        vec![[0, 1], [2, 3]],
        vec![],
        None,
        &[],
        &[],
        material(),
    )
    .unwrap();
    let state = model.initial_state();
    let contact = RodContact {
        activation_m: 0.1,
        stiffness_j: 0.005,
        clearance_m: 1e-9,
    };
    let (_, g, _) = contact.barrier(0.02).unwrap();
    for vertex in 0..4 {
        for axis in 0..3 {
            let mut plus = state.clone();
            let mut minus = state.clone();
            let h = 1e-7;
            plus.positions[vertex][axis] += h;
            minus.positions[vertex][axis] -= h;
            let finite = (model.contact_energy(&plus, contact).unwrap().0
                - model.contact_energy(&minus, contact).unwrap().0)
                / (2.0 * h);
            let expected = if axis == 2 {
                if vertex < 2 { -0.5 * g } else { 0.5 * g }
            } else {
                0.0
            };
            assert!(
                (finite - expected).abs() < 1e-8,
                "vertex {vertex} axis {axis}: {finite} vs {expected}"
            );
        }
    }
}

#[test]
fn orientation_sweeps_reduce_elastic_energy_on_fixed_randomized_rods() {
    let model = chain(&[0, 1, 2], &[]);
    let mut seed = 0x7b31_u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 11) as f64 / (1_u64 << 53) as f64) * 2.0 - 1.0
    };
    for method in [OrientationMethod::Approximate, OrientationMethod::Refined] {
        for _ in 0..80 {
            let mut state = model.initial_state();
            state.positions[1] += 0.1 * DVec3::new(random(), random(), random());
            state.positions[2] += 0.1 * DVec3::new(random(), random(), random());
            for q in &mut state.orientations {
                let axis = DVec3::new(random(), random(), random()).normalize_or_zero();
                *q = DQuat::from_axis_angle(
                    if axis == DVec3::ZERO { DVec3::X } else { axis },
                    random() * 2.5,
                );
            }
            let before = model.elastic_energy(&state).unwrap();
            let mut cfg = settings();
            cfg.iterations = 1;
            cfg.orientation_method = method;
            model.step(&mut state, cfg).unwrap();
            let after = model.elastic_energy(&state).unwrap();
            assert!(after <= before + 1e-10, "{method:?}: {before} -> {after}");
        }
    }
}

#[test]
fn repeated_close_rod_steps_remain_finite_and_separated() {
    use marvelous_cloth::cosserat::RodContact;
    let rest = vec![
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::new(0.0, -0.5, 0.04),
        DVec3::new(0.0, 0.5, 0.04),
    ];
    let model = RodModel::new(
        rest,
        vec![[0, 1], [2, 3]],
        vec![],
        None,
        &[0, 1],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    state.velocities[2] = DVec3::new(0.0, 0.0, -0.25);
    state.velocities[3] = state.velocities[2];
    let mut cfg = settings();
    cfg.dt_s = 0.01;
    cfg.iterations = 12;
    cfg.contact = Some(RodContact {
        activation_m: 0.05,
        stiffness_j: 0.002,
        clearance_m: 1e-7,
    });
    for i in 0..30 {
        let metrics = model
            .step(&mut state, cfg)
            .unwrap_or_else(|error| panic!("step {i}: {error}"));
        assert!(metrics.minimum_nonadjacent_distance_m.unwrap() > 1e-7);
        assert!(metrics.elastic_energy_j.is_finite() && metrics.contact_energy_j.is_finite());
        assert!(state.positions.iter().all(|p| p.is_finite()));
    }
}

#[test]
fn parallel_rod_contact_does_not_exert_tangential_force_in_straight_slide() {
    use marvelous_cloth::cosserat::RodContact;
    let points = vec![
        DVec3::new(-1.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(-0.25, 0.0, 0.04),
        DVec3::new(0.25, 0.0, 0.04),
    ];
    let model = RodModel::new(
        points,
        vec![[0, 1], [2, 3]],
        vec![],
        None,
        &[0, 1],
        &[],
        material(),
    )
    .unwrap();
    let mut state = model.initial_state();
    state.velocities[2] = DVec3::new(0.1, 0.0, 0.0);
    state.velocities[3] = state.velocities[2];
    let mut cfg = settings();
    cfg.dt_s = 0.01;
    cfg.iterations = 800;
    cfg.contact = Some(RodContact {
        activation_m: 0.05,
        stiffness_j: 0.002,
        clearance_m: 1e-7,
    });
    model.step(&mut state, cfg).unwrap();
    let momentum_x = (state.velocities[2].x + state.velocities[3].x) * 0.5;
    assert!(
        (momentum_x - 0.1).abs() < 1e-8,
        "tangential velocity {momentum_x}"
    );
}
