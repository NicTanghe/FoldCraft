//! Experimental rectangular welded-rod lattice in panel material space.
//!
//! This is a small research prototype, not a yarn weave: perpendicular rods
//! share vertices. No panel clipping, sewing, avatar contact or calibration is
//! provided. The material coordinates are the physical rest geometry.
use super::rod_attachment::SupportTriangle;
use crate::cosserat::{RodMaterial, RodModel};
use glam::{DVec2, DVec3};

const MAX_VERTICES: usize = 4096;

/// A rectangular rest-space lattice with an interpolation-only triangulation.
/// Indices are transient; never persist them in an authored garment.
pub struct RectRodGrid {
    pub model: RodModel,
    pub material_points: Vec<DVec2>,
    pub support: Vec<SupportTriangle>,
}

impl RectRodGrid {
    /// Build from panel-local metric coordinates and explicit physical rod
    /// parameters. `columns` and `rows` include the boundary vertices.
    /// Pins are runtime indices into the resulting row-major vertex array.
    pub fn new(
        min: DVec2,
        max: DVec2,
        columns: usize,
        rows: usize,
        material: RodMaterial,
        pins: &[usize],
    ) -> Result<Self, String> {
        let count = columns.checked_mul(rows).ok_or("Rod grid size overflow")?;
        if !min.is_finite()
            || !max.is_finite()
            || max.x <= min.x
            || max.y <= min.y
            || columns < 2
            || rows < 2
            || count > MAX_VERTICES
        {
            return Err("Rod grid needs a finite, nonempty rectangle and 2..4096 vertices".into());
        }
        let spacing = (max - min) / DVec2::new((columns - 1) as f64, (rows - 1) as f64);
        if !spacing.is_finite() || spacing.min_element() <= 1e-12 {
            return Err("Rod grid spacing is invalid or too small".into());
        }
        let mut material_points = Vec::with_capacity(count);
        for y in 0..rows {
            for x in 0..columns {
                material_points.push(min + spacing * DVec2::new(x as f64, y as f64));
            }
        }
        let mut edges = Vec::new();
        let mut joints = Vec::new();
        for y in 0..rows {
            let first = edges.len();
            for x in 0..columns - 1 {
                let a = y * columns + x;
                edges.push([a, a + 1]);
                if x > 0 {
                    joints.push([first + x - 1, first + x]);
                }
            }
        }
        for x in 0..columns {
            let first = edges.len();
            for y in 0..rows - 1 {
                let a = y * columns + x;
                edges.push([a, a + columns]);
                if y > 0 {
                    joints.push([first + y - 1, first + y]);
                }
            }
        }
        let mut support = Vec::with_capacity((columns - 1) * (rows - 1) * 2);
        for y in 0..rows - 1 {
            for x in 0..columns - 1 {
                let a = y * columns + x;
                let b = a + 1;
                let c = a + columns;
                let d = c + 1;
                support.push(SupportTriangle {
                    vertices: [a, b, d],
                });
                support.push(SupportTriangle {
                    vertices: [a, d, c],
                });
            }
        }
        let rest: Vec<DVec3> = material_points.iter().map(|uv| uv.extend(0.0)).collect();
        let model = RodModel::new(rest, edges, joints, None, pins, &[], material)?;
        Ok(Self {
            model,
            material_points,
            support,
        })
    }
}
