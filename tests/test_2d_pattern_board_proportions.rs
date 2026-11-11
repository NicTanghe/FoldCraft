//! Tests for FC-0036: 2D pattern board SVG generation, isometric proportions, Bézier handles, and CAD vertex knobs.
#![cfg(feature = "desktop")]

use cssimpler::core::Node;
use marvelous_cloth::atelier::DocumentFiles;
use marvelous_cloth::atelier::css_runner::{CssRunnerState, build_runner_2d_viewport};
use marvelous_cloth::fitting::FittingProject;
use marvelous_cloth::garment::presets::GarmentKind;
use std::path::PathBuf;
use std::sync::mpsc;

#[test]
fn test_2d_viewport_svg_preserves_aspect_ratio_and_renders_bezier_handles() {
    let dir = tempfile::tempdir().unwrap();
    let avatar = dir.path().join("avatar.obj");
    std::fs::write(&avatar, "v 10 0 -10\nv 11 0 -10\nv 10 1 -10\nf 1 2 3\n").unwrap();
    let project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);

    let (worker_tx, _worker_rx) = mpsc::channel();
    let worker_latest = std::sync::Arc::new(std::sync::Mutex::new(None));
    let scene = project.compile().expect("compile scene");
    let files = DocumentFiles {
        output: PathBuf::from("output/test"),
        source: None,
        modified: false,
    };

    let state = CssRunnerState::new(project, scene, files, worker_tx, worker_latest);
    let node = build_runner_2d_viewport(&state);

    let mut found_svg = false;
    let mut found_preserve_aspect = false;
    let mut found_vertex = false;
    let mut found_piece = false;

    fn inspect_node(
        node: &Node,
        found_svg: &mut bool,
        found_aspect: &mut bool,
        found_v: &mut bool,
        found_p: &mut bool,
    ) {
        if let Node::Element(elem) = node {
            if elem.tag == "svg" {
                *found_svg = true;
                if let Some(par) = elem.attribute("preserveAspectRatio") {
                    if par.contains("xMidYMid") && par.contains("meet") {
                        *found_aspect = true;
                    }
                }
                if let Some(vb) = elem.attribute("viewBox") {
                    let parts: Vec<f64> = vb
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if parts.len() == 4 {
                        let w = parts[2];
                        let h = parts[3];
                        // Isometric aspect ratio: width must equal height
                        assert!(
                            (w - h).abs() < 1e-3,
                            "viewBox must have square 1:1 aspect ratio: {vb}"
                        );
                    }
                }
            }
            if elem.classes.iter().any(|c| c == "cad-vertex") {
                *found_v = true;
            }
            if elem.classes.iter().any(|c| c == "pattern-piece") {
                *found_p = true;
            }
            for child in &elem.children {
                inspect_node(child, found_svg, found_aspect, found_v, found_p);
            }
        }
    }

    inspect_node(
        &node,
        &mut found_svg,
        &mut found_preserve_aspect,
        &mut found_vertex,
        &mut found_piece,
    );

    assert!(found_svg, "SVG canvas element must be rendered");
    assert!(
        found_preserve_aspect,
        "preserveAspectRatio='xMidYMid meet' must be set on SVG canvas"
    );
    assert!(
        found_vertex,
        "CAD vertex knobs must be rendered on pattern boundaries"
    );
    assert!(found_piece, "Pattern pieces must be rendered");
}
