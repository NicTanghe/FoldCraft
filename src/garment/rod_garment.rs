//! Research-only full garment rod truss compiled from panel rest triangles.
//!
//! This welded triangular network uses one explicitly supplied rod material;
//! it is not a calibrated fabric or a woven yarn model. It uses heuristic
//! bending joints. A separate research wrapper adds fixed-avatar surface
//! contact; grab support, production performance and UI integration are absent.
use super::{CompiledGarment, rod_sewing::compile_rod_stitches};
use crate::cosserat::{RodMaterial, RodModel, RodState};
use glam::{DMat3, DQuat, DVec3};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub struct GarmentRodNetwork {
    garment: Arc<CompiledGarment>,
    pub model: RodModel,
    edges: Vec<[usize; 2]>,
    edge_faces: Vec<Vec<usize>>,
    joint_count: usize,
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
        let mut index = BTreeMap::new();
        let mut incident = vec![Vec::new(); n];
        for (i, &[a, b]) in edges.iter().enumerate() {
            index.insert([a, b], i);
            incident[a].push(i);
            incident[b].push(i);
        }
        let mut edge_faces = vec![Vec::new(); edges.len()];
        for (face_id, face) in garment.mesh.triangles.iter().enumerate() {
            let [a, b, c] = face.vertices;
            for (u, v) in [(a, b), (b, c), (c, a)] {
                edge_faces[*index
                    .get(&[u.min(v), u.max(v)])
                    .ok_or("Missing panel edge")?]
                .push(face_id);
            }
        }
        if edge_faces
            .iter()
            .any(|faces| faces.is_empty() || faces.len() > 2)
            || incident.iter().any(|edges| edges.len() > 64)
        {
            return Err("Panel rod topology is nonmanifold or exceeds the joint budget".into());
        }
        // Pair near-opposite edges at each vertex into individual rod paths.
        // The pairing is a deterministic research approximation, not yarns.
        let mut joints = Vec::new();
        for (vertex, segments) in incident.iter().enumerate() {
            let mut candidates = Vec::new();
            for (i, &a) in segments.iter().enumerate() {
                let other_a = if edges[a][0] == vertex {
                    edges[a][1]
                } else {
                    edges[a][0]
                };
                let da = (garment.mesh.uv[other_a] - garment.mesh.uv[vertex]).normalize_or_zero();
                for &b in &segments[i + 1..] {
                    let other_b = if edges[b][0] == vertex {
                        edges[b][1]
                    } else {
                        edges[b][0]
                    };
                    let db =
                        (garment.mesh.uv[other_b] - garment.mesh.uv[vertex]).normalize_or_zero();
                    let dot = da.dot(db);
                    if dot < -0.5 {
                        candidates.push((dot, a, b));
                    }
                }
            }
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            let mut used = BTreeSet::new();
            for (_, a, b) in candidates {
                if !used.contains(&a) && !used.contains(&b) {
                    used.insert(a);
                    used.insert(b);
                    joints.push([a, b]);
                }
            }
        }
        let rest_frames: Vec<_> = edges
            .iter()
            .map(|[a, b]| {
                let direction = (garment.mesh.uv[*b] - garment.mesh.uv[*a]).extend(0.0);
                frame_from_tangent_normal(direction, DVec3::Z)
            })
            .collect::<Result<_, _>>()?;
        let joint_count = if material.bend_twist_rigidity > 0.0 {
            joints.len()
        } else {
            0
        };
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
            joints,
            Some(rest_frames),
            &pins,
            &[],
            material,
        )?
        .with_stitches(stitches)?;
        Ok(Self {
            garment,
            model,
            edges,
            edge_faces,
            joint_count,
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
            let mut normal = DVec3::ZERO;
            for &face_id in &self.edge_faces[index] {
                let [i, j, k] = self.garment.mesh.triangles[face_id].vertices;
                let uv = &self.garment.mesh.uv;
                let sign = (uv[j] - uv[i]).perp_dot(uv[k] - uv[i]).signum();
                normal += sign
                    * (state.positions[j] - state.positions[i])
                        .cross(state.positions[k] - state.positions[i]);
            }
            state.orientations[index] = frame_from_tangent_normal(tangent, normal)?;
        }
        self.model.elastic_energy(&state)?;
        Ok(state)
    }

    pub(crate) fn garment(&self) -> &CompiledGarment {
        &self.garment
    }

    pub fn bending_joint_count(&self) -> usize {
        self.joint_count
    }

    pub fn matches(&self, garment: &Arc<CompiledGarment>) -> bool {
        Arc::ptr_eq(&self.garment, garment)
    }
}

fn frame_from_tangent_normal(tangent: DVec3, normal: DVec3) -> Result<DQuat, String> {
    let length = tangent.length();
    if !length.is_finite() || length <= 1e-12 {
        return Err("Placed garment has a collapsed rod".into());
    }
    let x = tangent / length;
    let z_raw = normal - x * normal.dot(x);
    let z_length = z_raw.length();
    if !z_length.is_finite() || z_length <= 1e-18 {
        return Err("Placed garment has an undefined rod frame".into());
    }
    let z = z_raw / z_length;
    let y = z.cross(x);
    Ok(DQuat::from_mat3(&DMat3::from_cols(x, y, z)).normalize())
}
