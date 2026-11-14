use glam::{DVec2, DVec3};
use marvelous_cloth::{
    cosserat::{RodMaterial, RodSettings},
    garment::{
        GarmentTemplate,
        domain::Hole,
        rod_attachment::RodSurfaceMapping,
        rod_panel::PanelRodNetwork,
        shapes::{Primitive, primitive},
    },
};
use std::sync::Arc;
fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 0.01,
        stretch_shear_rigidity: 10.0,
        bend_twist_rigidity: 0.0,
    }
}
fn holed_garment() -> Arc<marvelous_cloth::garment::CompiledGarment> {
    let mut t = GarmentTemplate::two_panel_demo();
    t.version = 2;
    t.seams.clear();
    t.panels[0].pins.clear();
    let mut polygon = primitive(
        Primitive::Rectangle,
        DVec2::new(0.4, 0.6),
        90.,
        Default::default(),
    )
    .unwrap();
    polygon.holes.push(Hole {
        points: vec![
            DVec2::new(0.15, 0.2),
            DVec2::new(0.15, 0.4),
            DVec2::new(0.25, 0.4),
            DVec2::new(0.25, 0.2),
        ],
        edges: (0..4).map(|i| format!("hole_{i}")).collect(),
    });
    t.panels[0].polygon = Some(polygon);
    Arc::new(t.compile().unwrap())
}
#[test]
fn compiled_panel_support_preserves_holes_and_drives_its_surface() {
    let garment = holed_garment();
    let panel = &garment.panels[0];
    let network = PanelRodNetwork::compile(garment.clone(), &panel.id, material(), &[]).unwrap();
    let uv = &garment.mesh.uv[panel.vertices.clone()];
    assert_eq!(network.support.len(), panel.triangles.len());
    assert!(
        RodSurfaceMapping::bind(&panel.id, uv, &network.support, &[DVec2::new(0.2, 0.3)]).is_err()
    );
    let mut state = network.model.initial_state();
    assert!(network.model.elastic_energy(&state).unwrap() < 1e-20);
    network
        .model
        .step(
            &mut state,
            RodSettings {
                dt_s: 0.001,
                iterations: 100,
                gravity_m_s2: DVec3::new(0.0, 0.0, -10.0),
                drag_per_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
    let positions = network
        .mapping
        .evaluate(&garment, &state.positions)
        .unwrap();
    for (p, rest) in positions.iter().zip(uv) {
        assert!((*p - rest.extend(-0.00001)).length() < 1e-8);
    }
}
#[test]
fn rejects_missing_panels_and_invalid_pins() {
    let garment = holed_garment();
    assert!(PanelRodNetwork::compile(garment.clone(), "missing", material(), &[]).is_err());
    assert!(PanelRodNetwork::compile(garment, "left", material(), &[usize::MAX]).is_err());
}
