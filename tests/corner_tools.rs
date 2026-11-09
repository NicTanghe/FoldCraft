use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate, curves,
        meshing::MeshingSettings,
        shapes::{self, MirrorAxis, Primitive},
    },
};

fn fixture() -> GarmentTemplate {
    authoring::apply(
        &GarmentTemplate::two_panel_demo(),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::new(0.4, 0.5),
            sweep_degrees: 180.0,
            origin: DVec2::ZERO,
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}
fn command(t: &GarmentTemplate, panel: usize, radius_m: f64) -> Command {
    Command::RoundCorner {
        panel: t.panels[panel].id.clone(),
        edge: t.panels[panel].polygon.as_ref().unwrap().edges[1].clone(),
        radius_m,
    }
}
#[test]
fn ten_mm_fillet_has_expected_tangencies_and_circle_error() {
    let t = fixture();
    let old = t.panels[0].polygon.as_ref().unwrap();
    let names = old.edges.clone();
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    assert!(edit.reset_simulation);
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly.edges[0], names[0]);
    assert_eq!(poly.edges[2], names[1]);
    assert!(poly.points[1].distance(DVec2::new(0.39, 0.0)) < 1e-12);
    assert!(poly.points[2].distance(DVec2::new(0.4, 0.01)) < 1e-12);
    let controls = poly.curves[&poly.edges[1]];
    let cubic = [poly.points[1], controls[0], controls[1], poly.points[2]];
    let center = DVec2::new(0.39, 0.01);
    let max_error = (0..=10000)
        .map(|i| (curves::cubic(cubic, i as f64 / 10000.0).distance(center) - 0.01).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_error < 0.000_002_73, "error in metres: {max_error}");
    let mut history = History::default();
    history.record(t.clone());
    let undo = history.restore(&edit.template, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undo.template).unwrap(),
        serde_json::to_vec(&t).unwrap()
    );
    let redo = history.restore(&undo.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redo.template).unwrap(),
        serde_json::to_vec(&edit.template).unwrap()
    );
    let json = serde_json::to_vec(&redo.template).unwrap();
    let loaded: GarmentTemplate = serde_json::from_slice(&json).unwrap();
    loaded.compile().unwrap();
}
#[test]
fn rejects_invalid_and_referenced_inputs_without_mutation() {
    let t = fixture();
    let before = serde_json::to_vec(&t).unwrap();
    for r in [0.0, -0.01, f64::NAN, f64::INFINITY, 1.0] {
        assert!(authoring::apply(&t, command(&t, 0, r)).is_err());
        assert_eq!(before, serde_json::to_vec(&t).unwrap());
    }
    let mut referenced = t.clone();
    let poly = referenced.panels[0].polygon.as_mut().unwrap();
    poly.target_lengths.insert(poly.edges[1].clone(), 0.2);
    assert!(authoring::apply(&referenced, command(&referenced, 0, 0.01)).is_err());
    let rounded = authoring::apply(&t, command(&t, 0, 0.01)).unwrap().template;
    let poly = rounded.panels[0].polygon.as_ref().unwrap();
    assert!(
        authoring::apply(
            &rounded,
            Command::RoundCorner {
                panel: rounded.panels[0].id.clone(),
                edge: poly.edges[1].clone(),
                radius_m: 0.01
            }
        )
        .is_err()
    );
}
#[test]
fn linked_rounding_works_from_either_partner_and_refuses_partner_metadata() {
    for index in 0..2 {
        let base = fixture();
        let t = authoring::apply(
            &base,
            Command::LinkedMirror {
                panel: base.panels[0].id.clone(),
                axis: MirrorAxis::X,
            },
        )
        .unwrap()
        .template;
        let edit = authoring::apply(&t, command(&t, index, 0.01)).unwrap();
        let a = &edit.template.panels[index];
        let b = &edit.template.panels[1 - index];
        let mut expected = a.polygon.clone().unwrap();
        shapes::mirror(
            &mut expected,
            MirrorAxis::X,
            DVec2::new(a.pattern.width, a.pattern.height),
        );
        assert_eq!(
            serde_json::to_value(expected).unwrap(),
            serde_json::to_value(&b.polygon).unwrap()
        );
        let mut blocked = t.clone();
        let p = blocked.panels[1 - index].polygon.as_mut().unwrap();
        p.target_lengths.insert(p.edges[0].clone(), 0.1);
        let before = serde_json::to_vec(&blocked).unwrap();
        assert!(authoring::apply(&blocked, command(&blocked, index, 0.01)).is_err());
        assert_eq!(before, serde_json::to_vec(&blocked).unwrap());
    }
}

#[test]
fn linked_rounding_that_changes_material_frame_is_atomic() {
    let base = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.1),
                DVec2::new(0.3, 0.5),
                DVec2::new(0.0, 0.4),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let t = authoring::apply(
        &base,
        Command::LinkedMirror {
            panel: base.panels[0].id.clone(),
            axis: MirrorAxis::X,
        },
    )
    .unwrap()
    .template;
    let before = serde_json::to_vec(&t).unwrap();
    let error = authoring::apply(&t, command(&t, 0, 0.01)).err().unwrap();
    assert!(error.contains("material frame"), "{error}");
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
}

#[test]
fn rounding_preserves_automatic_board_origins() {
    let mut t = fixture();
    for _ in 0..2 {
        t = authoring::apply(
            &t,
            Command::Duplicate {
                panel: t.panels[0].id.clone(),
            },
        )
        .unwrap()
        .template;
    }
    for panel in &mut t.panels {
        panel.layout_2d = None;
    }
    let origins: Vec<_> = (0..t.panels.len())
        .map(|i| authoring::layout(&t, i))
        .collect();
    let edit = authoring::apply(&t, command(&t, 1, 0.01)).unwrap();
    for (i, origin) in origins.iter().enumerate() {
        assert_eq!(authoring::layout(&edit.template, i), *origin);
    }
    // A corner that changes the first piece's width must not move later pieces.
    let skewed = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.1),
                DVec2::new(0.3, 0.5),
                DVec2::new(0.0, 0.4),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let mut t = authoring::apply(
        &skewed,
        Command::Duplicate {
            panel: skewed.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    for panel in &mut t.panels {
        panel.layout_2d = None;
    }
    let next_origin = authoring::layout(&t, 1);
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    assert!(edit.template.panels[0].pattern.width < t.panels[0].pattern.width);
    assert_eq!(authoring::layout(&edit.template, 1), next_origin);
}

#[test]
fn generated_boundary_name_is_bounded_and_avoids_collisions() {
    let mut t = fixture();
    let poly = t.panels[0].polygon.as_mut().unwrap();
    poly.edges[0] = "corner_round_1".into();
    poly.edges[1] = "e".repeat(80);
    t.compile().unwrap();
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly.edges[1], "corner_round_2");
    assert_eq!(poly.edges[2], "e".repeat(80));
    edit.template.compile().unwrap();
}

#[test]
fn forty_five_degree_fillet_has_correct_tangencies_and_radius() {
    let t = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.0),
                DVec2::new(0.6, 0.2),
                DVec2::new(0.6, 0.5),
                DVec2::new(0.0, 0.5),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    let trim = 0.01 * (std::f64::consts::PI / 8.0).tan();
    let corner = DVec2::new(0.4, 0.0);
    let direction = DVec2::new(1.0, 1.0).normalize();
    assert!(poly.points[1].distance(corner - DVec2::X * trim) < 1e-12);
    assert!(poly.points[2].distance(corner + direction * trim) < 1e-12);
    let controls = poly.curves[&poly.edges[1]];
    let cubic = [poly.points[1], controls[0], controls[1], poly.points[2]];
    let center = poly.points[1] + DVec2::Y * 0.01;
    let error = (0..=10000)
        .map(|i| (curves::cubic(cubic, i as f64 / 10000.0).distance(center) - 0.01).abs())
        .fold(0.0_f64, f64::max);
    assert!(error < 0.000_002_73, "error in metres: {error}");
}
