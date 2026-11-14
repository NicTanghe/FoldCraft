use glam::{DVec2, DVec3};
use marvelous_cloth::garment::rod_attachment::{RodSurfaceMapping, SupportTriangle};

fn square() -> (Vec<DVec2>, Vec<SupportTriangle>) {
    (
        vec![DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y],
        vec![
            SupportTriangle {
                vertices: [0, 1, 2],
            },
            SupportTriangle {
                vertices: [0, 2, 3],
            },
        ],
    )
}

#[test]
fn mapped_points_follow_affine_motion_and_shared_edges() {
    let (uv, faces) = square();
    let points = vec![
        DVec2::new(0.75, 0.25),
        DVec2::new(0.1, 0.9),
        DVec2::splat(0.5),
        DVec2::ZERO,
        DVec2::ONE,
    ];
    let mapping = RodSurfaceMapping::bind("front", &uv, &faces, &points).unwrap();
    assert_eq!(mapping.panel(), "front");
    let transform = |p: DVec2| DVec3::new(2.0 + 3.0 * p.x - p.y, -4.0 + p.x + 2.0 * p.y, 1.0 + p.y);
    let positions: Vec<_> = uv.iter().copied().map(transform).collect();
    let mapped = mapping.evaluate(&positions).unwrap();
    for (actual, p) in mapped.iter().zip(points) {
        assert!((*actual - transform(p)).length() < 1e-12);
    }
}

#[test]
fn reject_uncovered_and_ambiguous_points_without_extrapolating() {
    let (mut uv, mut faces) = square();
    assert!(RodSurfaceMapping::bind("front", &uv, &faces, &[DVec2::new(2.0, 0.0)]).is_err());
    uv.extend([DVec2::ZERO, DVec2::X, DVec2::ONE]);
    faces.push(SupportTriangle {
        vertices: [4, 5, 6],
    });
    assert!(RodSurfaceMapping::bind("front", &uv, &faces, &[DVec2::new(0.75, 0.25)]).is_err());
}

#[test]
fn reject_invalid_support_and_incompatible_positions() {
    let (uv, faces) = square();
    let mapping = RodSurfaceMapping::bind("front", &uv, &faces, &[DVec2::splat(0.25)]).unwrap();
    assert!(mapping.evaluate(&[DVec3::ZERO; 3]).is_err());
    assert!(mapping.evaluate(&[DVec3::NAN; 4]).is_err());
    assert!(RodSurfaceMapping::bind("", &uv, &faces, &[]).is_err());
    assert!(
        RodSurfaceMapping::bind(
            "front",
            &uv,
            &[SupportTriangle {
                vertices: [0, 1, 4]
            }],
            &[]
        )
        .is_err()
    );
    assert!(
        RodSurfaceMapping::bind(
            "front",
            &uv,
            &[SupportTriangle {
                vertices: [0, 0, 1]
            }],
            &[]
        )
        .is_err()
    );
    assert!(
        RodSurfaceMapping::bind(
            "front",
            &[DVec2::ZERO, DVec2::X, DVec2::new(2.0, 0.0)],
            &[SupportTriangle {
                vertices: [0, 1, 2]
            }],
            &[]
        )
        .is_err()
    );
    assert!(RodSurfaceMapping::bind("front", &uv, &faces, &[DVec2::NAN]).is_err());
}

#[test]
fn mapped_surface_follows_an_actual_rod_solver_step() {
    use marvelous_cloth::cosserat::{RodMaterial, RodModel, RodSettings};
    let uv = vec![DVec2::ZERO, DVec2::X, DVec2::Y];
    let mapping = RodSurfaceMapping::bind(
        "panel",
        &uv,
        &[SupportTriangle {
            vertices: [0, 1, 2],
        }],
        &[DVec2::new(0.25, 0.25)],
    )
    .unwrap();
    let model = RodModel::new(
        uv.iter().map(|p| p.extend(0.0)).collect(),
        vec![[0, 1], [1, 2], [2, 0]],
        vec![],
        None,
        &[],
        &[],
        RodMaterial {
            linear_density: 1.0,
            stretch_shear_rigidity: 100.0,
            bend_twist_rigidity: 0.0,
        },
    )
    .unwrap();
    let mut state = model.initial_state();
    model
        .step(
            &mut state,
            RodSettings {
                dt_s: 0.01,
                gravity_m_s2: DVec3::new(0.0, 0.0, -10.0),
                drag_per_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
    let actual = mapping.evaluate(&state.positions).unwrap()[0];
    assert!((actual - DVec3::new(0.25, 0.25, -0.001)).length() < 1e-10);
}

#[test]
fn binds_a_real_garment_panel_and_rejects_stale_compilations() {
    use marvelous_cloth::garment::{GarmentTemplate, rod_attachment::GarmentRodMapping};
    use std::sync::Arc;
    let template = GarmentTemplate::two_panel_demo();
    let garment = Arc::new(template.compile().unwrap());
    let panel = &garment.panels[0];
    let range = panel.vertices.clone();
    let uv = &garment.mesh.uv[range.clone()];
    let faces: Vec<_> = garment.mesh.triangles[panel.triangles.clone()]
        .iter()
        .map(|triangle| SupportTriangle {
            vertices: triangle.vertices.map(|index| index - range.start),
        })
        .collect();
    let mapping = GarmentRodMapping::bind(garment.clone(), &panel.id, uv, &faces).unwrap();
    let positions: Vec<_> = uv.iter().map(|uv| DVec3::new(uv.x, uv.y, 3.0)).collect();
    let mapped = mapping.evaluate(&garment, &positions).unwrap();
    assert_eq!(mapped.len(), uv.len());
    for (actual, expected) in mapped.iter().zip(&positions) {
        assert!((*actual - *expected).length() < 1e-10);
    }
    assert!(mapping.matches(&garment, &panel.id));
    let weights = mapping.global_weights(&garment, 100).unwrap();
    assert_eq!(weights.len(), uv.len());
    assert!(
        weights
            .iter()
            .all(|row| (row.iter().map(|(_, w)| w).sum::<f64>() - 1.0).abs() < 1e-12)
    );
    assert!(weights.iter().flatten().all(|(i, _)| *i >= 100));
    assert!(mapping.global_weights(&garment, usize::MAX).is_err());
    let recompiled = Arc::new(template.compile().unwrap());
    assert!(!mapping.matches(&recompiled, &panel.id));
    assert!(mapping.evaluate(&recompiled, &positions).is_err());
    assert!(!mapping.matches(&garment, "another panel"));
    assert!(GarmentRodMapping::bind(garment.clone(), "missing", uv, &faces).is_err());
}
