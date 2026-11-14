use glam::DVec3;
use marvelous_cloth::cosserat::{RodContact, RodMaterial, RodModel, RodSettings, RodStitch};

fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 1.0,
        stretch_shear_rigidity: 20.0,
        bend_twist_rigidity: 0.0,
    }
}
fn model() -> RodModel {
    RodModel::new(
        vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::X + DVec3::Y],
        vec![[0, 1], [2, 3]],
        vec![],
        None,
        &[0, 1],
        &[],
        material(),
    )
    .unwrap()
}
fn stitch() -> RodStitch {
    RodStitch {
        terms: vec![(1, 1.0), (3, -1.0)],
        stiffness_n_per_m: 100.0,
    }
}
#[test]
fn stitches_pull_free_rod_toward_fixed_rod_and_report_energy_and_residual() {
    let model = model().with_stitches(vec![stitch()]).unwrap();
    let mut state = model.initial_state();
    let before = (state.positions[1] - state.positions[3]).length();
    let initial_energy = model.stitch_energy(&state).unwrap();
    let metrics = model
        .step(
            &mut state,
            RodSettings {
                dt_s: 0.1,
                iterations: 100,
                gravity_m_s2: DVec3::ZERO,
                drag_per_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
    assert!((state.positions[1] - state.positions[3]).length() < before);
    assert!(metrics.stitch_energy_j < initial_energy);
    assert!((metrics.stitch_energy_j - model.stitch_energy(&state).unwrap()).abs() < 1e-12);
    assert!(metrics.position_residual_rms_n.unwrap() < 1e-6);
}
#[test]
fn stitch_energy_is_objective_and_matches_force_by_finite_difference() {
    let model = model().with_stitches(vec![stitch()]).unwrap();
    let mut state = model.initial_state();
    let initial = model.stitch_energy(&state).unwrap();
    for p in &mut state.positions {
        *p += DVec3::new(5.0, -3.0, 2.0);
    }
    assert!((model.stitch_energy(&state).unwrap() - initial).abs() < 1e-12);
    let eps = 1e-6;
    state.positions[3].y += eps;
    let plus = model.stitch_energy(&state).unwrap();
    state.positions[3].y -= 2.0 * eps;
    let minus = model.stitch_energy(&state).unwrap();
    assert!(((plus - minus) / (2.0 * eps) - 100.0).abs() < 1e-7);
}
#[test]
fn invalid_stitches_are_rejected() {
    assert!(
        model()
            .with_stitches(vec![RodStitch {
                terms: vec![(1, 1.0), (3, 1.0)],
                stiffness_n_per_m: 1.0
            }])
            .is_err()
    );
    assert!(
        model()
            .with_stitches(vec![RodStitch {
                terms: vec![(3, -1.0), (1, 1.0)],
                stiffness_n_per_m: 1.0
            }])
            .is_err()
    );
    assert!(
        model()
            .with_stitches(vec![RodStitch {
                terms: vec![(1, 1.0), (9, -1.0)],
                stiffness_n_per_m: 1.0
            }])
            .is_err()
    );
}

#[test]
fn compiled_garment_seams_preserve_energy_after_weighted_rod_mapping() {
    use marvelous_cloth::garment::{GarmentTemplate, rod_sewing::compile_rod_stitches};
    let garment = GarmentTemplate::two_panel_demo().compile().unwrap();
    assert!(!garment.seams.is_empty());
    let n = garment.mesh.uv.len();
    let weights: Vec<_> = (0..n).map(|i| vec![(i, 0.3), (i + n, 0.7)]).collect();
    let stitches = compile_rod_stitches(&garment, 2 * n, &weights).unwrap();
    assert!(!stitches.is_empty());
    let positions: Vec<_> = garment
        .initial_state
        .x
        .iter()
        .copied()
        .chain(garment.initial_state.x.iter().copied())
        .collect();
    let actual: f64 = stitches
        .iter()
        .map(|s| {
            let gap: DVec3 = s.terms.iter().map(|(i, w)| positions[*i] * *w).sum();
            0.5 * s.stiffness_n_per_m * gap.length_squared()
        })
        .sum();
    let expected: f64 = garment
        .seams
        .iter()
        .map(|s| s.energy(&garment.initial_state.x))
        .sum();
    assert!(expected > 0.0);
    assert!((actual - expected).abs() < expected * 1e-12);
    let mut bad = weights.clone();
    let vertex = garment.seams[0].samples[0].point.terms[0].0;
    bad[vertex].clear();
    assert!(compile_rod_stitches(&garment, 2 * n, &bad).is_err());
    assert!(compile_rod_stitches(&garment, 2 * n, &weights[..n - 1]).is_err());
}

#[test]
fn real_garment_seam_samples_pull_panel_rod_networks_together() {
    use marvelous_cloth::garment::{GarmentTemplate, rod_sewing::compile_rod_stitches};
    use std::collections::BTreeSet;
    let garment = GarmentTemplate::two_panel_demo().compile().unwrap();
    let n = garment.mesh.uv.len();
    let weights: Vec<_> = (0..n).map(|i| vec![(i, 1.0)]).collect();
    let stitches = compile_rod_stitches(&garment, n, &weights).unwrap();
    let mut edges = BTreeSet::new();
    for t in &garment.mesh.triangles {
        let [a, b, c] = t.vertices;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            edges.insert([u.min(v), u.max(v)]);
        }
    }
    let model = RodModel::new(
        garment.mesh.uv.iter().map(|uv| uv.extend(0.0)).collect(),
        edges.into_iter().collect(),
        vec![],
        None,
        &[],
        &[],
        material(),
    )
    .unwrap()
    .with_stitches(stitches)
    .unwrap();
    let mut state = model.initial_state();
    state.positions.clone_from(&garment.initial_state.x);
    let initial = model.stitch_energy(&state).unwrap();
    let metrics = model
        .step(
            &mut state,
            RodSettings {
                dt_s: 0.01,
                iterations: 100,
                gravity_m_s2: DVec3::ZERO,
                drag_per_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(initial > 0.0);
    assert!(
        metrics.stitch_energy_j < initial,
        "before {initial}, after {}",
        metrics.stitch_energy_j
    );
}

#[test]
fn sewing_contact_exclusions_are_local_and_unrelated_contacts_remain_active() {
    let rest = vec![
        DVec3::ZERO,
        DVec3::X,
        DVec3::Y,
        DVec3::X + DVec3::Y,
        DVec3::Y * 1.2,
        DVec3::X + DVec3::Y * 1.2,
    ];
    let build = || {
        RodModel::new(
            rest.clone(),
            vec![[0, 1], [2, 3], [4, 5]],
            vec![],
            None,
            &[0, 1],
            &[],
            material(),
        )
        .unwrap()
    };
    let stitched = build().with_stitches(vec![stitch()]).unwrap();
    let contact = RodContact {
        activation_m: 0.3,
        stiffness_j: 0.01,
        clearance_m: 0.001,
    };
    let mut state = stitched.initial_state();
    let initial = state.clone();
    let contact_before = stitched.contact_energy(&state, contact).unwrap().0;
    assert!(contact_before > 0.0, "unrelated pair must remain active");
    let settings = RodSettings {
        dt_s: 0.01,
        iterations: 30,
        gravity_m_s2: DVec3::ZERO,
        drag_per_s: 0.0,
        contact: Some(contact),
        ..Default::default()
    };
    let metrics = stitched.step(&mut state, settings).unwrap();
    assert!(metrics.stitch_energy_j < stitched.stitch_energy(&initial).unwrap());
    assert!(metrics.contact_energy_j > 0.0);
    let inertia: f64 = state
        .positions
        .iter()
        .zip(&initial.positions)
        .skip(2)
        .map(|(new, old)| 0.5 * 0.5 / settings.dt_s.powi(2) * (*new - *old).length_squared())
        .sum();
    let before_objective = stitched.elastic_energy(&initial).unwrap()
        + stitched.stitch_energy(&initial).unwrap()
        + contact_before;
    let after_objective =
        metrics.elastic_energy_j + metrics.stitch_energy_j + metrics.contact_energy_j + inertia;
    assert!(
        after_objective <= before_objective + 1e-9,
        "objective increased: {before_objective} -> {after_objective}"
    );
    assert!(state.positions.iter().all(|p| p.is_finite()));
    let mut overlapping = initial.clone();
    overlapping.positions[2] = overlapping.positions[0];
    overlapping.positions[3] = overlapping.positions[1];
    assert!(stitched.contact_energy(&overlapping, contact).is_ok());
    assert!(build().contact_energy(&overlapping, contact).is_err());
    overlapping.positions[4] = overlapping.positions[0];
    overlapping.positions[5] = overlapping.positions[1];
    let before = overlapping.clone();
    assert!(stitched.step(&mut overlapping, settings).is_err());
    assert_eq!(overlapping, before);
}
