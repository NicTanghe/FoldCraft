use glam::{DVec2, DVec3};
use marvelous_cloth::{
    cosserat::{RodMaterial, RodSettings},
    garment::{rod_attachment::RodSurfaceMapping, rod_grid::RectRodGrid},
};

fn material() -> RodMaterial {
    RodMaterial {
        linear_density: 0.01,
        stretch_shear_rigidity: 10.0,
        bend_twist_rigidity: 0.001,
    }
}

#[test]
fn rectangle_grid_drives_attached_surface_in_free_fall() {
    let grid = RectRodGrid::new(
        DVec2::new(-1.0, -0.5),
        DVec2::new(1.0, 0.5),
        4,
        3,
        material(),
        &[],
    )
    .unwrap();
    assert_eq!(grid.material_points.len(), 12);
    assert_eq!(grid.support.len(), 12);
    let points = [
        DVec2::new(-1.0, -0.5),
        DVec2::new(0.0, 0.0),
        DVec2::new(0.75, 0.2),
        DVec2::new(1.0, 0.5),
    ];
    let mapping =
        RodSurfaceMapping::bind("panel", &grid.material_points, &grid.support, &points).unwrap();
    let mut state = grid.model.initial_state();
    grid.model
        .step(
            &mut state,
            RodSettings {
                dt_s: 0.01,
                iterations: 100,
                gravity_m_s2: DVec3::new(0.0, 0.0, -10.0),
                drag_per_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
    let mapped = mapping.evaluate(&state.positions).unwrap();
    for (p, actual) in points.iter().zip(mapped) {
        assert!(
            (actual - p.extend(-0.001)).length() < 1e-9,
            "point {p:?}: {actual:?}, error {}",
            (actual - p.extend(-0.001)).length()
        );
    }
}

#[test]
fn invalid_rectangles_resolutions_and_pins_are_rejected() {
    assert!(RectRodGrid::new(DVec2::ZERO, DVec2::ONE, 1, 3, material(), &[]).is_err());
    assert!(RectRodGrid::new(DVec2::ONE, DVec2::ZERO, 3, 3, material(), &[]).is_err());
    assert!(RectRodGrid::new(DVec2::ZERO, DVec2::ONE, usize::MAX, 3, material(), &[]).is_err());
    assert!(RectRodGrid::new(DVec2::ZERO, DVec2::ONE, 100, 100, material(), &[]).is_err());
    assert!(RectRodGrid::new(DVec2::ZERO, DVec2::ONE, 2, 2, material(), &[4]).is_err());
}

#[test]
fn welded_orthogonal_grid_has_an_unresisted_crossing_angle_mode() {
    let grid = RectRodGrid::new(DVec2::ZERO, DVec2::ONE, 3, 3, material(), &[]).unwrap();
    let mut state = grid.model.initial_state();
    let angle: f64 = 0.3;
    for (position, rest) in state.positions.iter_mut().zip(&grid.material_points) {
        *position = DVec3::new(rest.x + rest.y * angle.sin(), rest.y * angle.cos(), 0.0);
    }
    let vertical =
        glam::DQuat::from_rotation_arc(DVec3::X, DVec3::new(angle.sin(), angle.cos(), 0.0));
    for orientation in &mut state.orientations[6..] {
        *orientation = vertical;
    }
    assert!(grid.model.elastic_energy(&state).unwrap() < 1e-20);
}
