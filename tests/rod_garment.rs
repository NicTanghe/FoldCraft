use glam::DVec3;
use marvelous_cloth::{
    cosserat::{RodMaterial, RodSettings},
    garment::{GarmentTemplate, rod_garment::GarmentRodNetwork},
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
    let network = GarmentRodNetwork::compile(garment, material()).unwrap();
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
