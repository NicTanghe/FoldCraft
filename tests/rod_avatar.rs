use glam::DVec3;
use marvelous_cloth::{
    avatar::SurfaceMesh,
    cosserat::{RodMaterial, RodSettings},
    garment::{GarmentTemplate, rod_avatar::GarmentRodAvatar, rod_garment::GarmentRodNetwork},
    ogc::OgcSettings,
};
use std::sync::Arc;

fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 0.1,
        stretch_shear_rigidity: 10.0,
        bend_twist_rigidity: 0.001,
    }
}
fn garment(z: f64) -> Arc<marvelous_cloth::garment::CompiledGarment> {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    t.panels[0].pins.clear();
    t.panels[0].pattern.nx = 2;
    t.panels[0].pattern.ny = 2;
    t.panels[0].placement.translation = DVec3::new(0.0, 0.0, z);
    Arc::new(t.compile().unwrap())
}
fn plane() -> SurfaceMesh {
    SurfaceMesh {
        vertices: vec![
            DVec3::new(-2., -2., 0.),
            DVec3::new(2., -2., 0.),
            DVec3::new(2., 2., 0.),
            DVec3::new(-2., 2., 0.),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}
fn contact() -> OgcSettings {
    OgcSettings {
        radius: 0.02,
        stiffness: 100.0,
        ..Default::default()
    }
}

#[test]
fn fixed_surface_contact_prevents_fall_through_a_plane() {
    let garment = garment(0.03);
    let network = GarmentRodNetwork::compile(garment.clone(), material()).unwrap();
    let mut state = network.initial_state().unwrap();
    let mut solver = GarmentRodAvatar::new(network, &plane(), &state, contact()).unwrap();
    let control = GarmentRodNetwork::compile(garment, material()).unwrap();
    let mut free = control.initial_state().unwrap();
    let settings = RodSettings {
        dt_s: 0.01,
        iterations: 2,
        gravity_m_s2: DVec3::new(0.0, 0.0, -10.0),
        drag_per_s: 0.0,
        ..Default::default()
    };
    let mut saw_contact = false;
    for _ in 0..20 {
        let metrics = solver.step(&mut state, settings).unwrap();
        saw_contact |= metrics.surface_contact_energy_j > 0.0;
        control.model.step(&mut free, settings).unwrap();
    }
    assert!(saw_contact);
    assert!(state.positions.iter().all(|p| p.z > 0.0));
    assert!(free.positions.iter().any(|p| p.z < 0.0));
}

#[test]
fn intersecting_initial_surface_is_rejected() {
    let garment = garment(0.0);
    let network = GarmentRodNetwork::compile(garment, material()).unwrap();
    let state = network.initial_state().unwrap();
    assert!(GarmentRodAvatar::new(network, &plane(), &state, contact()).is_err());
}

#[test]
fn high_speed_motion_is_bounded_and_invalid_steps_are_atomic() {
    let garment = garment(0.03);
    let network = GarmentRodNetwork::compile(garment, material()).unwrap();
    let mut state = network.initial_state().unwrap();
    for v in &mut state.velocities {
        *v = DVec3::new(0.0, 0.0, -100.0);
    }
    let mut solver = GarmentRodAvatar::new(network, &plane(), &state, contact()).unwrap();
    let settings = RodSettings {
        dt_s: 0.01,
        iterations: 2,
        gravity_m_s2: DVec3::ZERO,
        drag_per_s: 0.0,
        ..Default::default()
    };
    let metrics = solver.step(&mut state, settings).unwrap();
    assert!(metrics.bounded_updates > 0);
    assert!(state.positions.iter().all(|p| p.z > 0.0));
    state.time_s = -1.0;
    let invalid = state.clone();
    assert!(solver.step(&mut state, settings).is_err());
    assert_eq!(state, invalid);
}

#[test]
fn active_surface_contact_pushes_away_without_gravity() {
    let garment = garment(0.015);
    let network = GarmentRodNetwork::compile(garment, material()).unwrap();
    let mut state = network.initial_state().unwrap();
    let before = state.positions.clone();
    let mut solver = GarmentRodAvatar::new(network, &plane(), &state, contact()).unwrap();
    let settings = RodSettings {
        dt_s: 0.005,
        iterations: 5,
        gravity_m_s2: DVec3::ZERO,
        drag_per_s: 0.0,
        ..Default::default()
    };
    let metrics = solver.step(&mut state, settings).unwrap();
    assert!(metrics.surface_contact_energy_j > 0.0);
    let total: f64 = state
        .positions
        .iter()
        .zip(before)
        .map(|(p, q)| p.z - q.z)
        .sum();
    assert!(total > 0.0, "surface forces should push the cloth away");
}

#[test]
fn sewn_two_panel_garment_steps_with_avatar_and_rod_contact() {
    use marvelous_cloth::cosserat::RodContact;
    let mut t = GarmentTemplate::two_panel_demo();
    for panel in &mut t.panels {
        panel.placement.rotation_degrees = DVec3::ZERO;
        panel.placement.translation.z = 0.03;
    }
    let garment = Arc::new(t.compile().unwrap());
    let network = GarmentRodNetwork::compile(garment, material()).unwrap();
    let mut state = network.initial_state().unwrap();
    let before = network.model.stitch_energy(&state).unwrap();
    let mut solver = GarmentRodAvatar::new(network, &plane(), &state, contact()).unwrap();
    let settings = RodSettings {
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
    };
    let metrics = solver.step(&mut state, settings).unwrap();
    assert!(metrics.stitch_energy_j < before);
    assert!(state.positions.iter().all(|p| p.z > 0.0));
}
