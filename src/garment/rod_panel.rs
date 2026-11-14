//! Experimental rod network built from a compiled panel's material triangles.
//!
//! Edges become rods and triangles are interpolation cells. This is a welded
//! triangular truss, not woven yarn. It has no rod bending joints, material
//! calibration, sewing, avatar collision or automatic draped-state conversion.
use super::{
    CompiledGarment,
    rod_attachment::{GarmentRodMapping, SupportTriangle},
};
use crate::cosserat::{RodMaterial, RodModel};
use std::{collections::BTreeSet, sync::Arc};

/// Runtime-only research topology for one exact compiled garment panel.
pub struct PanelRodNetwork {
    pub model: RodModel,
    pub mapping: GarmentRodMapping,
    pub support: Vec<SupportTriangle>,
}

impl PanelRodNetwork {
    /// Retains existing rest-space mesh resolution and boundary constraints.
    /// `pins` are local runtime rod vertex indices, not authored identifiers.
    pub fn compile(
        garment: Arc<CompiledGarment>,
        panel: &str,
        material: RodMaterial,
        pins: &[usize],
    ) -> Result<Self, String> {
        let entry = garment
            .panels
            .iter()
            .find(|p| p.id == panel)
            .ok_or_else(|| format!("Unknown panel {panel}"))?;
        let vertices = entry.vertices.clone();
        let uvs = garment
            .mesh
            .uv
            .get(vertices.clone())
            .ok_or("Invalid panel vertex range")?;
        let triangles = garment
            .mesh
            .triangles
            .get(entry.triangles.clone())
            .ok_or("Invalid panel triangle range")?;
        if uvs.len() < 3 || uvs.len() > 4096 || triangles.is_empty() {
            return Err(
                "Panel rod network requires 3..4096 vertices and at least one triangle".into(),
            );
        }
        let mut edges = BTreeSet::new();
        let mut support = Vec::with_capacity(triangles.len());
        for triangle in triangles {
            if triangle.vertices.iter().any(|v| !vertices.contains(v)) {
                return Err("Panel triangle contains a vertex outside its panel".into());
            }
            let ids = triangle.vertices.map(|v| v - vertices.start);
            for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
                if a == b {
                    return Err("Panel has a degenerate triangle".into());
                }
                edges.insert([a.min(b), a.max(b)]);
            }
            support.push(SupportTriangle { vertices: ids });
        }
        let model = RodModel::new(
            uvs.iter().map(|uv| uv.extend(0.0)).collect(),
            edges.into_iter().collect(),
            vec![],
            None,
            pins,
            &[],
            material,
        )?;
        let mapping = GarmentRodMapping::bind(garment.clone(), panel, uvs, &support)?;
        Ok(Self {
            model,
            mapping,
            support,
        })
    }
}
