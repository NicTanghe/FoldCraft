use glam::DVec3;
use marvelous_cloth::{
    avatar::SurfaceMesh,
    cosserat::{RodMaterial, RodSettings},
    garment::{
        GarmentTemplate, presets::GarmentKind, rod_avatar::GarmentRodAvatar,
        rod_garment::GarmentRodNetwork,
    },
    ogc::OgcSettings,
};
use std::sync::Arc;
fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 1.0,
        stretch_shear_rigidity: 20.0,
        bend_twist_rigidity: 0.0,
    }
}

#[test]
fn default_tshirt_advances_with_interactive_rod_budget() {
    let garment = Arc::new(
        marvelous_cloth::garment::presets::basic(GarmentKind::Tshirt, 2)
            .compile()
            .unwrap(),
    );
    let material =
        marvelous_cloth::garment::rod_garment::approximate_rod_material(&garment).unwrap();
    let network = GarmentRodNetwork::compile(garment.clone(), material).unwrap();
    let mut state = network.state_from_mesh(&garment.initial_state).unwrap();
    assert_eq!(state.positions, garment.initial_state.x);
    assert_eq!(state.velocities, garment.initial_state.v);
    let avatar =
        SurfaceMesh::load_obj(std::path::Path::new("assets/makehuman/mannequin.obj")).unwrap();
    let mut solver =
        GarmentRodAvatar::new(network, &avatar, &state, OgcSettings::default()).unwrap();
    let settings = RodSettings {
        dt_s: 1.0 / 60.0,
        iterations: 4,
        drag_per_s: 1.5,
        ..Default::default()
    };
    for step in 1..=10 {
        let before = state.positions.clone();
        let metrics = solver.step(&mut state, settings).unwrap();
        let max_motion = state
            .positions
            .iter()
            .zip(&before)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0_f64, f64::max);
        eprintln!(
            "default shirt rod step {step}: {:.1} ms, max motion {:.4} m",
            metrics.step_ms, max_motion
        );
        assert!((state.time_s - step as f64 * settings.dt_s).abs() < 1e-12);
        assert!(max_motion > if step == 1 { 0.01 } else { 1e-6 });
        assert!(metrics.surface_minimum_distance_m.unwrap() > 0.0);
    }
}

#[test]
fn whole_garment_network_transfers_seams_pins_and_placement() {
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let network = GarmentRodNetwork::compile(garment.clone(), material()).unwrap();
    assert!(network.matches(&garment));
    let mut state = network.initial_state().unwrap();
    assert_eq!(state.positions, garment.initial_state.x);
    let original = state.clone();
    let seam_before = network.model.stitch_energy(&state).unwrap();
    let garment_energy: f64 = garment
        .seams
        .iter()
        .map(|seam| seam.energy(&state.positions))
        .sum();
    assert!((seam_before - garment_energy).abs() < 1e-12);
    assert!(network.model.elastic_energy(&state).unwrap() < 1e-18);
    let metrics = network
        .model
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
    assert!(metrics.stitch_energy_j < seam_before);
    for (i, pin) in garment.mesh.pins.iter().enumerate() {
        if pin.is_some() {
            assert_eq!(state.positions[i], original.positions[i]);
        }
    }
}

#[test]
fn malformed_ranges_and_collapsed_placement_are_rejected() {
    let template = GarmentTemplate::two_panel_demo();
    let mut invalid = template.compile().unwrap();
    invalid.panels[1].triangles = invalid.panels[0].triangles.clone();
    assert!(GarmentRodNetwork::compile(Arc::new(invalid), material()).is_err());
    let mut invalid = template.compile().unwrap();
    let edge = invalid.mesh.triangles[0].vertices;
    invalid.initial_state.x[edge[1]] = invalid.initial_state.x[edge[0]];
    let network = GarmentRodNetwork::compile(Arc::new(invalid), material()).unwrap();
    assert!(network.initial_state().is_err());
}

#[test]
fn whole_garment_can_step_with_stitches_and_experimental_self_contact() {
    use marvelous_cloth::cosserat::RodContact;
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let mut bending = material();
    bending.bend_twist_rigidity = 0.001;
    let network = GarmentRodNetwork::compile(garment, bending).unwrap();
    let mut state = network.initial_state().unwrap();
    let result = network.model.step(
        &mut state,
        RodSettings {
            dt_s: 0.0001,
            iterations: 1,
            gravity_m_s2: DVec3::ZERO,
            drag_per_s: 0.0,
            contact: Some(RodContact {
                activation_m: 0.005,
                stiffness_j: 0.001,
                clearance_m: 1e-8,
            }),
            ..Default::default()
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(state.positions.iter().all(|p| p.is_finite()));
}

#[test]
fn bending_joints_are_objective_and_respond_to_curvature() {
    use glam::DQuat;
    let mut bent = GarmentTemplate::two_panel_demo().compile().unwrap();
    for p in &mut bent.initial_state.x {
        p.z += 0.4 * p.x * p.x;
    }
    let mut rigidly_moved = bent.clone();
    let rotation = DQuat::from_rotation_y(0.7) * DQuat::from_rotation_x(-0.3);
    for p in &mut rigidly_moved.initial_state.x {
        *p = rotation * *p + DVec3::new(2.0, -3.0, 1.0);
    }
    let mut bending = material();
    bending.bend_twist_rigidity = 0.02;
    let a = GarmentRodNetwork::compile(Arc::new(bent.clone()), bending).unwrap();
    let b = GarmentRodNetwork::compile(Arc::new(rigidly_moved), bending).unwrap();
    let without = GarmentRodNetwork::compile(Arc::new(bent), material()).unwrap();
    assert!(a.bending_joint_count() > 0);
    assert_eq!(without.bending_joint_count(), 0);
    let energy = a.model.elastic_energy(&a.initial_state().unwrap()).unwrap();
    let rotated = b.model.elastic_energy(&b.initial_state().unwrap()).unwrap();
    let no_bending = without
        .model
        .elastic_energy(&without.initial_state().unwrap())
        .unwrap();
    assert!(
        energy - no_bending > 1e-7,
        "bending contribution {}",
        energy - no_bending
    );
    assert!((energy - rotated).abs() < 1e-9, "{energy} vs {rotated}");
}

#[test]
fn rod_checkpoint_roundtrips_and_rejects_changed_model_transactionally() {
    use marvelous_cloth::cosserat::{OrientationMethod, RodCheckpoint};
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let mut bending = material();
    bending.bend_twist_rigidity = 0.02;
    let network = GarmentRodNetwork::compile(garment.clone(), bending).unwrap();
    let mut original = network.initial_state().unwrap();
    let settings = RodSettings {
        dt_s: 0.001,
        iterations: 5,
        gravity_m_s2: DVec3::ZERO,
        drag_per_s: 0.0,
        orientation_method: OrientationMethod::Refined,
        ..Default::default()
    };
    network.model.step(&mut original, settings).unwrap();
    let checkpoint = network.model.checkpoint(&original).unwrap();
    let serialized = serde_json::to_vec(&checkpoint).unwrap();
    let decoded: RodCheckpoint = serde_json::from_slice(&serialized).unwrap();
    let mut resumed = network.initial_state().unwrap();
    network
        .model
        .restore_checkpoint(&mut resumed, &decoded)
        .unwrap();
    assert_eq!(original, resumed);
    network.model.step(&mut original, settings).unwrap();
    network.model.step(&mut resumed, settings).unwrap();
    assert_eq!(original, resumed);
    let incompatible = GarmentRodNetwork::compile(garment, material()).unwrap();
    let mut target = incompatible.initial_state().unwrap();
    let before = target.clone();
    assert!(
        incompatible
            .model
            .restore_checkpoint(&mut target, &decoded)
            .is_err()
    );
    assert_eq!(before, target);
    let mut json: serde_json::Value = serde_json::from_slice(&serialized).unwrap();
    json["version"] = 2.into();
    let unknown: RodCheckpoint = serde_json::from_value(json).unwrap();
    assert!(
        network
            .model
            .restore_checkpoint(&mut resumed, &unknown)
            .is_err()
    );
}

#[test]
fn rigidly_placed_panels_start_without_artificial_bending() {
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let mut bending = material();
    bending.bend_twist_rigidity = 0.02;
    let network = GarmentRodNetwork::compile(garment, bending).unwrap();
    let state = network.initial_state().unwrap();
    assert!(network.bending_joint_count() > 0);
    let energy = network.model.elastic_energy(&state).unwrap();
    assert!(energy < 1e-18, "unexpected rest energy: {energy}");
}

#[test]
fn approximate_material_responds_to_shared_fabric_properties() {
    use marvelous_cloth::garment::rod_garment::approximate_rod_material;
    let mut template = GarmentTemplate::two_panel_demo();
    let base = approximate_rod_material(&template.compile().unwrap()).unwrap();
    let fabric = &mut template.fabrics[0].properties;
    fabric.density *= 2.0;
    fabric.warp *= 2.0;
    fabric.weft *= 2.0;
    fabric.shear *= 2.0;
    fabric.bend_warp *= 2.0;
    fabric.bend_weft *= 2.0;
    let doubled = approximate_rod_material(&template.compile().unwrap()).unwrap();
    assert!((doubled.linear_density / base.linear_density - 2.0).abs() < 1e-12);
    assert!((doubled.stretch_shear_rigidity / base.stretch_shear_rigidity - 2.0).abs() < 1e-12);
    assert!((doubled.bend_twist_rigidity / base.bend_twist_rigidity - 2.0).abs() < 1e-12);
}
