//! SI units throughout: metres, kilograms, seconds, Newtons.
pub mod authoring;
pub mod avatar;
pub mod avatar_import;
pub mod avatar_measurements;
pub mod contact;
#[cfg(feature = "desktop")]
#[path = "atelier/css_ui/mod.rs"]
pub mod css_ui;
pub mod document;
pub mod energy;
pub mod fabrics;
pub mod fitting;
pub mod garment;
pub mod gltf_avatar;
#[cfg(feature = "desktop")]
pub mod gpu_vbd;
pub mod issue_report;
pub mod math;
/// Opt-in static mesh sizing; evaluated during preparation, never solver steps.
pub mod mesh_resolution;
pub mod model;
pub mod motion;
pub mod ogc;
pub mod pattern_import;
pub mod picking;
pub mod project;
/// Shared optional renderer; the numerical modules do not depend on this module.
#[cfg(feature = "desktop")]
pub mod render;
pub mod session_protocol;
pub mod solver;
pub mod solver_lab;
pub mod template_catalog;
pub mod vbd;
/// Logical viewport coordinates and metric-board navigation, independent of UI.
pub mod viewport_controller;
/// Typed workspace actions and capability checks, independent of desktop UI.
pub mod workspace_commands;
