//! Research-only full garment rod truss compiled from panel rest triangles.
//!
//! This welded triangular network uses one explicitly supplied rod material;
//! it is not a calibrated fabric or a woven yarn model. It has no bending
//! joints, avatar contact, grab support, production broad phase, or UI adapter.
use super::{CompiledGarment, rod_sewing::compile_rod_stitches};
use crate::cosserat::{RodMaterial, RodModel, RodState};
use glam::{DQuat, DVec3};
use std::{collections::BTreeSet, sync::Arc};

pub struct GarmentRodNetwork {
    garment: Arc<CompiledGarment>,
    pub model: RodModel,
    edges: Vec<[usize; 2]>,
}

impl GarmentRodNetwork {
    /// Reuse the compiled triangle edges as a rod truss. The rest positions
    /// come exclusively from the 2D material coordinates. Sewing quadrature
    /// and compiled pins are transferred to the same runtime vertex order.
    pub fn compile(garment: Arc<CompiledGarment>, material: RodMaterial) -> Result<Self, String> {
        let n = garment.mesh.uv.len();
        if !(3..=4096).contains(&n) || garment.mesh.pins.len() != n {
            return Err("Rod garment requires 3..4096 vertices and compatible pins".into());
        }
        let mut edges = BTreeSet::new();
        let mut covered_vertices = vec![false; n];
        let mut covered_triangles = vec![false; garment.mesh.triangles.len()];
        for panel in &garment.panels {
            let panel_vertices = covered_vertices
                .get_mut(panel.vertices.clone())
                .ok_or("Invalid panel vertex range")?;
            if panel_vertices.is_empty() || panel_vertices.iter().any(|v| *v) {
                return Err("Panel vertex ranges are empty or overlap".into());
            }
            panel_vertices.fill(true);
            let panel_triangles = covered_triangles
                .get_mut(panel.triangles.clone())
                .ok_or("Invalid panel triangle range")?;
            if panel_triangles.is_empty() || panel_triangles.iter().any(|v| *v) {
                return Err("Panel triangle ranges are empty or overlap".into());
            }
            panel_triangles.fill(true);
            let faces = garment
                .mesh
                .triangles
                .get(panel.triangles.clone())
                .ok_or("Invalid panel triangle range")?;
            for face in faces {
                if face
                    .vertices
                    .iter()
                    .any(|i| !panel.vertices.contains(i) || *i >= n)
                {
                    return Err("Panel triangle does not belong to its panel".into());
                }
                let [a, b, c] = face.vertices;
                for (a, b) in [(a, b), (b, c), (c, a)] {
                    if a == b {
                        return Err("Invalid garment triangle".into());
                    }
                    edges.insert([a.min(b), a.max(b)]);
                }
            }
        }
        if covered_triangles.iter().any(|v| !*v) || covered_vertices.iter().any(|v| !*v) {
            return Err("Panel ranges do not cover garment".into());
        }
        let edges: Vec<_> = edges.into_iter().collect();
        let pins: Vec<_> = garment
            .mesh
            .pins
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.is_some().then_some(i))
            .collect();
        let weights: Vec<_> = (0..n).map(|i| vec![(i, 1.0)]).collect();
        let stitches = compile_rod_stitches(&garment, n, &weights)?;
        let model = RodModel::new(
            garment.mesh.uv.iter().map(|uv| uv.extend(0.0)).collect(),
            edges.clone(),
            vec![],
            None,
            &pins,
            &[],
            material,
        )?
        .with_stitches(stitches)?;
        Ok(Self {
            garment,
            model,
            edges,
        })
    }

    /// Initialize from the compiled garment's placed 3D state while retaining
    /// material rest lengths. Recompiling the garment requires a new network.
    pub fn initial_state(&self) -> Result<RodState, String> {
        let source = &self.garment.initial_state;
        if source.x.len() != self.garment.mesh.uv.len()
            || source.v.len() != source.x.len()
            || !source.time.is_finite()
            || source.x.iter().chain(&source.v).any(|p| !p.is_finite())
        {
            return Err("Compiled garment initial state is invalid".into());
        }
        let mut state = self.model.initial_state();
        state.positions.clone_from(&source.x);
        state.velocities.clone_from(&source.v);
        state.time_s = source.time;
        for (index, [a, b]) in self.edges.iter().copied().enumerate() {
            let tangent = state.positions[b] - state.positions[a];
            let length = tangent.length();
            if !length.is_finite() || length <= 1e-12 {
                return Err("Placed garment has a collapsed rod".into());
            }
            state.orientations[index] = DQuat::from_rotation_arc(DVec3::X, tangent / length);
        }
        self.model.elastic_energy(&state)?;
        Ok(state)
    }

    pub fn matches(&self, garment: &Arc<CompiledGarment>) -> bool {
        Arc::ptr_eq(&self.garment, garment)
    }
}
