//! Editable garment templates compile into a fixed, CPU-only physical model.
//! No mesh indices are persisted in the authoring format.
pub mod appearance;
pub mod curves;
pub mod darts;
pub mod diagnostics;
pub mod domain;
pub mod dxf_import;
pub mod embedding;
pub mod export;
pub mod freehand;
pub mod graphics;
pub mod markings;
pub mod meshing;
pub mod pins;
pub mod polygon;
pub mod presets;
pub mod production_metrics;
pub mod rod_attachment;
pub mod rod_avatar;
pub mod rod_garment;
pub mod rod_grid;
pub mod rod_panel;
pub mod rod_sewing;
pub mod seam;
pub mod shapes;
pub mod svg_import;
pub mod template;
pub mod trims;

pub use template::{CompiledGarment, GarmentTemplate};
