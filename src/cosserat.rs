//! Experimental CPU stable Cosserat rods for explicitly authored rod geometry.
//!
//! The position and quaternion update are adapted from Jerry Hsu's MIT-licensed
//! StableCosseratRods, commit 11081ed22ceb3f8824f447b64557745895676d0a;
//! see `THIRD_PARTY_NOTICES.md` for the complete notice. This module is not a
//! garment solver or a physical implementation of the geometric fiber guides.

use std::collections::HashSet;
use std::time::Instant;

use glam::{DMat3, DQuat, DVec3, DVec4};

/// Homogeneous, isotropic rod parameters in SI units.
#[derive(Clone, Copy, Debug)]
pub struct RodMaterial {
    /// Mass per unit rest length, kg/m.
    pub linear_density: f64,
    /// Stretch/shear rigidity, N. Segment energy uses `rigidity * length`.
    pub stretch_shear_rigidity: f64,
    /// Bend/twist rigidity, N m². A joint uses `4 * rigidity / mean_length`.
    pub bend_twist_rigidity: f64,
}

#[derive(Clone, Debug)]
struct Segment {
    vertices: [usize; 2],
    rest_length: f64,
    rest_frame: DQuat,
    stretch: f64,
    pinned: bool,
}

#[derive(Clone, Debug)]
struct Joint {
    segments: [usize; 2],
    rest_relative: DQuat,
    stiffness: f64,
}

/// A quadratic positional stitch. Coefficients describe the difference between
/// two material attachments and must sum to zero; stiffness is in N/m.
#[derive(Clone, Debug)]
pub struct RodStitch {
    pub terms: Vec<(usize, f64)>,
    pub stiffness_n_per_m: f64,
}

impl RodStitch {
    fn gap(&self, positions: &[DVec3]) -> DVec3 {
        self.terms.iter().map(|(i, w)| positions[*i] * *w).sum()
    }
}

/// Compiled rod network. Vertex and segment indices are runtime-only and must
/// not be stored as persistent garment identifiers.
#[derive(Clone, Debug)]
pub struct RodModel {
    rest_positions: Vec<DVec3>,
    inverse_masses: Vec<f64>,
    segments: Vec<Segment>,
    joints: Vec<Joint>,
    incident_segments: Vec<Vec<usize>>,
    incident_joints: Vec<Vec<usize>>,
    stitches: Vec<RodStitch>,
    incident_stitches: Vec<Vec<(usize, f64)>>,
    sewing_exclusions: HashSet<(usize, usize)>,
}

/// Accepted state, including the orientation and exact-solve history. A state
/// must be retained between steps; positions alone are not a complete restart.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RodState {
    pub positions: Vec<DVec3>,
    pub velocities: Vec<DVec3>,
    pub orientations: Vec<DQuat>,
    /// Per-segment fixed-point history; used only by the refined update.
    pub orientation_gamma: Vec<f64>,
    pub time_s: f64,
}

/// Portable research checkpoint tied to the exact compiled rod model. This is
/// not a garment project cache and does not store timestep/contact settings.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RodCheckpoint {
    version: u32,
    model_signature: Vec<u64>,
    state: RodState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationMethod {
    /// Equation 22: lambda = |v| + |b|.
    Approximate,
    /// One persistent fixed-point update per orientation sweep (Algorithm 1).
    Refined,
}

#[derive(Clone, Copy, Debug)]
pub struct RodSettings {
    pub dt_s: f64,
    pub iterations: usize,
    pub gravity_m_s2: DVec3,
    /// Linear velocity drag in s⁻¹, applied after the implicit step.
    pub drag_per_s: f64,
    pub orientation_method: OrientationMethod,
    /// Optional all-pairs rod self-contact. Shared-endpoint pairs are excluded.
    pub contact: Option<RodContact>,
}

/// Parameters of the paper's segment-distance barrier. `activation_m` is an
/// activation distance, not a hard rod diameter. `clearance_m` is a numerical
/// exclusion around zero, not a certified physical thickness.
#[derive(Clone, Copy, Debug)]
pub struct RodContact {
    pub activation_m: f64,
    /// Barrier energy scale in joules.
    pub stiffness_j: f64,
    pub clearance_m: f64,
}

impl Default for RodSettings {
    fn default() -> Self {
        Self {
            dt_s: 1.0 / 120.0,
            iterations: 8,
            gravity_m_s2: DVec3::new(0.0, -9.81, 0.0),
            drag_per_s: 0.1,
            orientation_method: OrientationMethod::Approximate,
            contact: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RodMetrics {
    pub time_s: f64,
    pub dt_s: f64,
    /// Complete elapsed CPU time for the accepted call, milliseconds.
    pub step_ms: f64,
    pub elastic_energy_j: f64,
    /// Quadratic sewing energy, J, separate from rod elastic energy.
    pub stitch_energy_j: f64,
    /// Final unconstrained positional residual, RMS over free scalar DOFs, N.
    pub position_residual_rms_n: Option<f64>,
    pub max_stretch_shear: f64,
    pub contact_energy_j: f64,
    /// Minimum computed closest-pair distance; nearly parallel pairs use the
    /// existing floating-point closest-feature heuristic, not exact predicates.
    pub minimum_nonadjacent_distance_m: Option<f64>,
    pub bounded_updates: usize,
    pub rejected_line_searches: usize,
}

fn valid_quat(q: DQuat) -> bool {
    q.is_finite() && (q.length_squared() - 1.0).abs() <= 1e-8
}

fn qvec(q: DQuat) -> DVec4 {
    DVec4::new(q.x, q.y, q.z, q.w)
}

fn quat(v: DVec4) -> DQuat {
    DQuat::from_xyzw(v.x, v.y, v.z, v.w)
}

fn unordered(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

impl RodModel {
    /// Compile an explicitly supplied rest network. `rest_frames`, if supplied,
    /// rotate the local +X axis onto each corresponding rest segment. With
    /// `None`, a rotation from +X to each rest tangent is chosen; these frames
    /// and their relative rest rotations are then fixed for the model lifetime.
    /// Joint pairs must refer to distinct segments sharing a vertex.
    pub fn new(
        rest_positions: Vec<DVec3>,
        edges: Vec<[usize; 2]>,
        joint_pairs: Vec<[usize; 2]>,
        rest_frames: Option<Vec<DQuat>>,
        pinned_vertices: &[usize],
        pinned_segments: &[usize],
        material: RodMaterial,
    ) -> Result<Self, String> {
        if rest_positions.len() < 2
            || edges.is_empty()
            || rest_positions.iter().any(|p| !p.is_finite())
        {
            return Err("rod requires finite rest vertices and at least one segment".into());
        }
        if !material.linear_density.is_finite()
            || material.linear_density <= 0.0
            || !material.stretch_shear_rigidity.is_finite()
            || material.stretch_shear_rigidity <= 0.0
            || !material.bend_twist_rigidity.is_finite()
            || material.bend_twist_rigidity < 0.0
        {
            return Err("rod density and stretch rigidity must be positive; bend rigidity must be nonnegative".into());
        }
        if rest_frames.as_ref().is_some_and(|q| q.len() != edges.len()) {
            return Err("rest frame count does not match segments".into());
        }
        let mut pin_v = vec![false; rest_positions.len()];
        for &i in pinned_vertices {
            let pin = pin_v.get_mut(i).ok_or("pinned vertex out of bounds")?;
            *pin = true;
        }
        let mut pin_s = vec![false; edges.len()];
        for &i in pinned_segments {
            let pin = pin_s.get_mut(i).ok_or("pinned segment out of bounds")?;
            *pin = true;
        }
        let mut masses = vec![0.0; rest_positions.len()];
        let mut incident_segments = vec![Vec::new(); rest_positions.len()];
        let mut segments = Vec::with_capacity(edges.len());
        let mut unique_edges = HashSet::new();
        for (i, &[a, b]) in edges.iter().enumerate() {
            if a >= rest_positions.len()
                || b >= rest_positions.len()
                || a == b
                || !unique_edges.insert(unordered(a, b))
            {
                return Err("invalid, repeated, or out-of-bounds rod segment".into());
            }
            let delta = rest_positions[b] - rest_positions[a];
            let length = delta.length();
            if !length.is_finite() || length <= 1e-12 {
                return Err("rod rest segment length must exceed 1e-12 m".into());
            }
            let tangent = delta / length;
            let frame = rest_frames.as_ref().map_or_else(
                || DQuat::from_rotation_arc(DVec3::X, tangent),
                |frames| frames[i],
            );
            if !valid_quat(frame) || (frame * DVec3::X - tangent).length() > 1e-7 {
                return Err(
                    "rest frame must be unit length and aligned with its rest segment".into(),
                );
            }
            let mass = 0.5 * material.linear_density * length;
            let stretch = material.stretch_shear_rigidity * length;
            if !mass.is_finite() || mass <= 0.0 || !stretch.is_finite() || stretch <= 0.0 {
                return Err("rod segment mass or stiffness is not finite and positive".into());
            }
            masses[a] += mass;
            masses[b] += mass;
            incident_segments[a].push(i);
            incident_segments[b].push(i);
            segments.push(Segment {
                vertices: [a, b],
                rest_length: length,
                rest_frame: frame,
                stretch,
                pinned: pin_s[i],
            });
        }
        let mut inverse_masses = Vec::with_capacity(masses.len());
        for (i, mass) in masses.into_iter().enumerate() {
            if !mass.is_finite() || mass <= 0.0 {
                return Err("every rod vertex must have finite positive incident mass".into());
            }
            let inverse = if pin_v[i] { 0.0 } else { 1.0 / mass };
            if !inverse.is_finite() {
                return Err("rod inverse mass overflow".into());
            }
            inverse_masses.push(inverse);
        }
        let mut incident_joints = vec![Vec::new(); segments.len()];
        let mut joints = Vec::with_capacity(joint_pairs.len());
        let mut unique_joints = HashSet::new();
        for [a, b] in joint_pairs {
            if a >= segments.len()
                || b >= segments.len()
                || a == b
                || !unique_joints.insert(unordered(a, b))
            {
                return Err("invalid, repeated, or out-of-bounds rod joint".into());
            }
            let sa = &segments[a];
            let sb = &segments[b];
            if !sa.vertices.iter().any(|v| sb.vertices.contains(v)) {
                return Err("joint segments must share a vertex".into());
            }
            let stiffness =
                4.0 * material.bend_twist_rigidity / (0.5 * (sa.rest_length + sb.rest_length));
            if !stiffness.is_finite() {
                return Err("rod joint stiffness overflow".into());
            }
            if stiffness > 0.0 {
                let i = joints.len();
                joints.push(Joint {
                    segments: [a, b],
                    rest_relative: sa.rest_frame.conjugate() * sb.rest_frame,
                    stiffness,
                });
                incident_joints[a].push(i);
                incident_joints[b].push(i);
            }
        }
        let incident_stitches = vec![Vec::new(); rest_positions.len()];
        Ok(Self {
            stitches: Vec::new(),
            incident_stitches,
            sewing_exclusions: HashSet::new(),
            rest_positions,
            inverse_masses,
            segments,
            joints,
            incident_segments,
            incident_joints,
        })
    }

    /// Return a model with validated stitches. Changes require rebuilding the
    /// model; indices refer only to this runtime topology.
    pub fn with_stitches(mut self, stitches: Vec<RodStitch>) -> Result<Self, String> {
        let mut incident = vec![Vec::new(); self.rest_positions.len()];
        let mut exclusions = HashSet::new();
        for (index, stitch) in stitches.iter().enumerate() {
            let sum: f64 = stitch.terms.iter().map(|(_, w)| *w).sum();
            let magnitude: f64 = stitch.terms.iter().map(|(_, w)| w.abs()).sum();
            if !stitch.stiffness_n_per_m.is_finite()
                || stitch.stiffness_n_per_m <= 0.0
                || !(2..=64).contains(&stitch.terms.len())
                || !sum.is_finite()
                || !magnitude.is_finite()
                || magnitude <= 0.0
                || sum.abs() > 1e-10 * magnitude
                || stitch.terms.windows(2).any(|w| w[0].0 >= w[1].0)
                || stitch
                    .terms
                    .iter()
                    .any(|(i, w)| *i >= incident.len() || !w.is_finite() || *w == 0.0)
            {
                return Err("invalid rod stitch coefficients or stiffness".into());
            }
            for &(vertex, weight) in &stitch.terms {
                incident[vertex].push((index, weight));
                for &(other, other_weight) in &stitch.terms {
                    if (weight > 0.0 && other_weight < 0.0) || (weight < 0.0 && other_weight > 0.0)
                    {
                        exclusions.insert(unordered(vertex, other));
                    }
                }
            }
        }
        self.sewing_exclusions = exclusions;
        self.stitches = stitches;
        self.incident_stitches = incident;
        Ok(self)
    }

    /// Energy in the positional stitches, in joules.
    pub fn stitch_energy(&self, state: &RodState) -> Result<f64, String> {
        self.validate_state(state)?;
        let energy: f64 = self
            .stitches
            .iter()
            .map(|s| 0.5 * s.stiffness_n_per_m * s.gap(&state.positions).length_squared())
            .sum();
        if energy.is_finite() {
            Ok(energy)
        } else {
            Err("rod stitch energy overflow".into())
        }
    }

    pub fn initial_state(&self) -> RodState {
        RodState {
            positions: self.rest_positions.clone(),
            velocities: vec![DVec3::ZERO; self.rest_positions.len()],
            orientations: self.segments.iter().map(|s| s.rest_frame).collect(),
            orientation_gamma: vec![1.0; self.segments.len()],
            time_s: 0.0,
        }
    }

    fn validate_state(&self, state: &RodState) -> Result<(), String> {
        if state.positions.len() != self.rest_positions.len()
            || state.velocities.len() != self.rest_positions.len()
            || state.orientations.len() != self.segments.len()
            || state.orientation_gamma.len() != self.segments.len()
            || !state.time_s.is_finite()
            || state.time_s < 0.0
            || state
                .positions
                .iter()
                .chain(&state.velocities)
                .any(|v| !v.is_finite())
            || state.orientations.iter().any(|&q| !valid_quat(q))
            || state.orientation_gamma.iter().any(|v| !v.is_finite())
        {
            return Err("invalid rod state dimensions or non-finite/non-unit state".into());
        }
        Ok(())
    }

    // Exact model witness, not a lossy hash. Version changes must accompany
    // changes to these fields or their physical interpretation.
    fn model_signature(&self) -> Vec<u64> {
        fn vector(out: &mut Vec<u64>, v: DVec3) {
            out.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
        }
        fn quat_bits(out: &mut Vec<u64>, q: DQuat) {
            out.extend([q.x.to_bits(), q.y.to_bits(), q.z.to_bits(), q.w.to_bits()]);
        }
        let mut out = Vec::new();
        out.push(self.rest_positions.len() as u64);
        for p in &self.rest_positions {
            vector(&mut out, *p);
        }
        for mass in &self.inverse_masses {
            out.push(mass.to_bits());
        }
        out.push(self.segments.len() as u64);
        for seg in &self.segments {
            out.extend([
                seg.vertices[0] as u64,
                seg.vertices[1] as u64,
                seg.rest_length.to_bits(),
                seg.stretch.to_bits(),
                u64::from(seg.pinned),
            ]);
            quat_bits(&mut out, seg.rest_frame);
        }
        out.push(self.joints.len() as u64);
        for joint in &self.joints {
            out.extend([
                joint.segments[0] as u64,
                joint.segments[1] as u64,
                joint.stiffness.to_bits(),
            ]);
            quat_bits(&mut out, joint.rest_relative);
        }
        out.push(self.stitches.len() as u64);
        for stitch in &self.stitches {
            out.extend([
                stitch.terms.len() as u64,
                stitch.stiffness_n_per_m.to_bits(),
            ]);
            for (vertex, weight) in &stitch.terms {
                out.extend([*vertex as u64, weight.to_bits()]);
            }
        }
        out
    }

    pub fn checkpoint(&self, state: &RodState) -> Result<RodCheckpoint, String> {
        self.validate_state(state)?;
        Ok(RodCheckpoint {
            version: 1,
            model_signature: self.model_signature(),
            state: state.clone(),
        })
    }

    /// Restores only an exact matching model and leaves `state` unchanged on error.
    pub fn restore_checkpoint(
        &self,
        state: &mut RodState,
        checkpoint: &RodCheckpoint,
    ) -> Result<(), String> {
        if checkpoint.version != 1 || checkpoint.model_signature != self.model_signature() {
            return Err("Rod checkpoint does not match the compiled model".into());
        }
        self.validate_state(&checkpoint.state)?;
        *state = checkpoint.state.clone();
        Ok(())
    }

    /// Elastic strain energy at an accepted or candidate state, joules.
    pub fn elastic_energy(&self, state: &RodState) -> Result<f64, String> {
        self.validate_state(state)?;
        let mut energy = 0.0;
        for (i, seg) in self.segments.iter().enumerate() {
            let [a, b] = seg.vertices;
            let c = (state.positions[b] - state.positions[a]) / seg.rest_length
                - state.orientations[i] * DVec3::X;
            energy += 0.5 * seg.stretch * c.length_squared();
        }
        for joint in &self.joints {
            let [a, b] = joint.segments;
            let relative = state.orientations[a].conjugate() * state.orientations[b];
            let phi = if qvec(relative).dot(qvec(joint.rest_relative)) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let c = qvec(relative) - phi * qvec(joint.rest_relative);
            energy += 0.5 * joint.stiffness * c.length_squared();
        }
        if energy.is_finite() {
            Ok(energy)
        } else {
            Err("rod energy overflow".into())
        }
    }

    /// Optional segment-pair contact energy and minimum nonadjacent distance.
    /// Shared-endpoint pairs are excluded, including their immediate vicinity.
    pub fn contact_energy(
        &self,
        state: &RodState,
        contact: RodContact,
    ) -> Result<(f64, Option<f64>), String> {
        self.validate_state(state)?;
        contact.validate()?;
        self.contact_stats(&state.positions, contact)
    }

    /// Advance one implicit step transactionally. Optional contact handles
    /// nonadjacent rod segment pairs. There is no friction or angular inertia;
    /// the last accepted state survives an error.
    pub fn step(&self, state: &mut RodState, settings: RodSettings) -> Result<RodMetrics, String> {
        let started = Instant::now();
        self.validate_state(state)?;
        if !settings.dt_s.is_finite()
            || settings.dt_s <= 0.0
            || settings.iterations == 0
            || !settings.gravity_m_s2.is_finite()
            || !settings.drag_per_s.is_finite()
            || settings.drag_per_s < 0.0
            || settings.drag_per_s * settings.dt_s > 1.0
        {
            return Err("invalid rod timestep, iterations, gravity, or drag".into());
        }
        if let Some(contact) = settings.contact {
            contact.validate()?;
            self.contact_stats(&state.positions, contact)?;
        }
        let h = settings.dt_s;
        let mut next = state.clone();
        let mut bounded_updates = 0;
        let mut rejected_line_searches = 0;
        let mut prediction = state.positions.clone();
        for (i, p) in prediction.iter_mut().enumerate() {
            if self.inverse_masses[i] > 0.0 {
                *p += h * state.velocities[i] + h * h * settings.gravity_m_s2;
                if settings.contact.is_none() {
                    next.positions[i] += h * state.velocities[i];
                }
            } else {
                next.velocities[i] = DVec3::ZERO;
            }
        }
        for _ in 0..settings.iterations {
            for i in 0..next.positions.len() {
                let inv_mass = self.inverse_masses[i];
                if inv_mass == 0.0 {
                    continue;
                }
                let inertia = 1.0 / (inv_mass * h * h);
                let mut force = inertia * (prediction[i] - next.positions[i]);
                let mut hessian = inertia;
                for &s in &self.incident_segments[i] {
                    let seg = &self.segments[s];
                    let [a, b] = seg.vertices;
                    let c = (next.positions[b] - next.positions[a]) / seg.rest_length
                        - next.orientations[s] * DVec3::X;
                    let direction = if i == a { 1.0 } else { -1.0 };
                    force += direction * seg.stretch / seg.rest_length * c;
                    hessian += seg.stretch / (seg.rest_length * seg.rest_length);
                }
                for &(index, weight) in &self.incident_stitches[i] {
                    let stitch = &self.stitches[index];
                    force -= stitch.stiffness_n_per_m * weight * stitch.gap(&next.positions);
                    hessian += stitch.stiffness_n_per_m * weight * weight;
                }
                let mut matrix = DMat3::from_diagonal(DVec3::splat(hessian));
                if let Some(contact) = settings.contact {
                    self.for_each_contact_pair(&next.positions, |a, b, sample| {
                        if let Some(weight) = sample.vertex_weight(i, a, b) {
                            let (_, gradient, curvature) = contact.barrier(sample.distance)?;
                            force -= gradient * weight * sample.normal;
                            let n = sample.normal * weight;
                            matrix +=
                                DMat3::from_cols(n * n.x, n * n.y, n * n.z) * curvature.max(0.0);
                        }
                        Ok(())
                    })?;
                }
                let delta = if settings.contact.is_some() {
                    matrix.inverse() * force
                } else {
                    force / hessian
                };
                if !delta.is_finite() {
                    return Err("rod position solve became non-finite".into());
                }
                if let Some(contact) = settings.contact {
                    let mut fraction: f64 = 1.0;
                    let distance = delta.length();
                    self.for_each_contact_pair(&next.positions, |a, b, sample| {
                        if a.contains(&i) || b.contains(&i) {
                            if sample.safe_distance <= contact.clearance_m {
                                return Err("rod segment separation cannot be established above numerical clearance".into());
                            }
                            if distance > 0.0 {
                                fraction = fraction.min(0.8 * (sample.safe_distance - contact.clearance_m) / distance);
                            }
                        }
                        Ok(())
                    })?;
                    if fraction < 1.0 {
                        bounded_updates += 1;
                    }
                    fraction = fraction.clamp(0.0, 1.0);
                    let original = next.positions[i];
                    let before = self.local_objective(i, &next, &prediction, h, contact)?;
                    let mut accepted = false;
                    for _ in 0..32 {
                        next.positions[i] = original + fraction * delta;
                        match self.local_objective(i, &next, &prediction, h, contact) {
                            Ok(after) if after <= before + 1e-12 * before.abs().max(1e-12) => {
                                accepted = true;
                                break;
                            }
                            _ => {
                                fraction *= 0.5;
                            }
                        }
                    }
                    if !accepted {
                        next.positions[i] = original;
                        rejected_line_searches += 1;
                    }
                } else {
                    let updated = next.positions[i] + delta;
                    if !updated.is_finite() {
                        return Err("rod position solve became non-finite".into());
                    }
                    next.positions[i] = updated;
                }
            }
            for (i, seg) in self.segments.iter().enumerate() {
                if seg.pinned {
                    continue;
                }
                let [a, b] = seg.vertices;
                let tangent = next.positions[b] - next.positions[a];
                let v = (-2.0 * seg.stretch / seg.rest_length) * tangent;
                if !v.is_finite() || v.length_squared() <= 1e-30 {
                    return Err("rod orientation solve has a collapsed segment".into());
                }
                let mut bend = DVec4::ZERO;
                for &j in &self.incident_joints[i] {
                    let joint = &self.joints[j];
                    let first = joint.segments[0] == i;
                    let other = if first {
                        joint.segments[1]
                    } else {
                        joint.segments[0]
                    };
                    let other_q = next.orientations[other];
                    let mut relative = other_q.conjugate() * next.orientations[i];
                    if first {
                        relative = relative.conjugate();
                    }
                    let phi = if qvec(relative).dot(qvec(joint.rest_relative)) > 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let rest = if first {
                        joint.rest_relative.conjugate()
                    } else {
                        joint.rest_relative
                    };
                    bend += (joint.stiffness * phi) * qvec(other_q * rest);
                }
                let vlen = v.length();
                let blen = bend.length();
                if blen <= 1e-30 {
                    next.orientations[i] =
                        DQuat::from_rotation_arc(DVec3::X, tangent / tangent.length());
                    continue;
                }
                let vbex = qvec(
                    DQuat::from_xyzw(v.x, v.y, v.z, 0.0)
                        * quat(bend)
                        * DQuat::from_xyzw(1.0, 0.0, 0.0, 0.0),
                );
                let lambda = match settings.orientation_method {
                    OrientationMethod::Approximate => vlen + blen,
                    OrientationMethod::Refined => {
                        let old = vlen + blen * next.orientation_gamma[i].clamp(1e-3, 1.0);
                        let refined = ((vbex + old * bend).length() + vlen * vlen).sqrt();
                        next.orientation_gamma[i] = (refined - vlen) / blen;
                        refined
                    }
                };
                let q = vbex + lambda * bend;
                let magnitude = q.length();
                if !magnitude.is_finite() || magnitude <= 1e-30 {
                    return Err("rod orientation update is undefined".into());
                }
                next.orientations[i] = quat(q / magnitude);
            }
        }
        for i in 0..next.positions.len() {
            if self.inverse_masses[i] > 0.0 {
                next.velocities[i] = (next.positions[i] - state.positions[i])
                    * ((1.0 - settings.drag_per_s * h) / h);
            }
        }
        next.time_s += h;
        self.validate_state(&next)?;
        let energy = self.elastic_energy(&next)?;
        let stitch_energy_j = self.stitch_energy(&next)?;
        let mut max_strain: f64 = 0.0;
        let mut residual_sum = 0.0;
        let mut free = 0;
        let mut residuals = vec![DVec3::ZERO; next.positions.len()];
        for (i, r) in residuals.iter_mut().enumerate() {
            if self.inverse_masses[i] > 0.0 {
                *r = (next.positions[i] - prediction[i]) / (self.inverse_masses[i] * h * h);
                free += 1;
            }
        }
        for (i, seg) in self.segments.iter().enumerate() {
            let [a, b] = seg.vertices;
            let c = (next.positions[b] - next.positions[a]) / seg.rest_length
                - next.orientations[i] * DVec3::X;
            max_strain = max_strain.max(c.length());
            let f = seg.stretch / seg.rest_length * c;
            residuals[a] -= f;
            residuals[b] += f;
        }
        for stitch in &self.stitches {
            let force = stitch.stiffness_n_per_m * stitch.gap(&next.positions);
            for &(vertex, weight) in &stitch.terms {
                residuals[vertex] += weight * force;
            }
        }
        let (contact_energy_j, minimum_nonadjacent_distance_m) =
            if let Some(contact) = settings.contact {
                self.for_each_contact_pair(&next.positions, |a, b, sample| {
                    let (_, derivative, _) = contact.barrier(sample.distance)?;
                    for vertex in [a[0], a[1], b[0], b[1]] {
                        if let Some(weight) = sample.vertex_weight(vertex, a, b) {
                            residuals[vertex] += derivative * weight * sample.normal;
                        }
                    }
                    Ok(())
                })?;
                self.contact_stats(&next.positions, contact)?
            } else {
                (0.0, None)
            };
        for (i, r) in residuals.into_iter().enumerate() {
            if self.inverse_masses[i] > 0.0 {
                residual_sum += r.length_squared();
            }
        }
        let residual = (free > 0).then(|| (residual_sum / (3 * free) as f64).sqrt());
        if !max_strain.is_finite() || residual.is_some_and(|r| !r.is_finite()) {
            return Err("rod diagnostics overflow".into());
        }
        let time_s = next.time_s;
        *state = next;
        let metrics = RodMetrics {
            time_s,
            dt_s: h,
            step_ms: started.elapsed().as_secs_f64() * 1000.0,
            elastic_energy_j: energy,
            stitch_energy_j,
            position_residual_rms_n: residual,
            max_stretch_shear: max_strain,
            contact_energy_j,
            minimum_nonadjacent_distance_m,
            bounded_updates,
            rejected_line_searches,
        };
        Ok(metrics)
    }
}

// Include all four boundary minima, plus the interior minimum when the
// directions are numerically independent. The near-parallel branch is still
// an f64 approximation; the projected interval gap is used for motion bounds.
fn closest_rod_segments(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> (f64, f64) {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let aa = u.length_squared();
    let ee = v.length_squared();
    let bb = u.dot(v);
    let cc = u.dot(w);
    let ff = v.dot(w);
    let mut candidates = [
        (0.0, (ff / ee).clamp(0.0, 1.0)),
        (1.0, ((bb + ff) / ee).clamp(0.0, 1.0)),
        ((-cc / aa).clamp(0.0, 1.0), 0.0),
        (((bb - cc) / aa).clamp(0.0, 1.0), 1.0),
        (0.0, 0.0),
    ];
    let den = aa * ee - bb * bb;
    if den > 1e-14 * aa * ee {
        let s = (bb * ff - cc * ee) / den;
        let t = (aa * ff - bb * cc) / den;
        if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
            candidates[4] = (s, t);
        }
    }
    candidates
        .into_iter()
        .min_by(|&(s1, t1), &(s2, t2)| {
            (w + s1 * u - t1 * v)
                .length_squared()
                .total_cmp(&(w + s2 * u - t2 * v).length_squared())
        })
        .expect("fixed set of segment-distance candidates")
}

#[derive(Clone, Copy)]
struct ContactSample {
    distance: f64,
    safe_distance: f64,
    normal: DVec3,
    s: f64,
    t: f64,
}

impl ContactSample {
    fn vertex_weight(&self, vertex: usize, a: [usize; 2], b: [usize; 2]) -> Option<f64> {
        if vertex == a[0] {
            Some(1.0 - self.s)
        } else if vertex == a[1] {
            Some(self.s)
        } else if vertex == b[0] {
            Some(-(1.0 - self.t))
        } else if vertex == b[1] {
            Some(-self.t)
        } else {
            None
        }
    }
}

impl RodContact {
    fn validate(self) -> Result<(), String> {
        if !self.activation_m.is_finite()
            || self.activation_m <= 0.0
            || !self.stiffness_j.is_finite()
            || self.stiffness_j <= 0.0
            || !self.clearance_m.is_finite()
            || self.clearance_m < 0.0
            || self.clearance_m >= self.activation_m
        {
            return Err("rod contact requires positive activation and stiffness and smaller nonnegative clearance".into());
        }
        Ok(())
    }

    /// Paper Eq. 30: energy (J), distance gradient (N), curvature (N/m).
    pub fn barrier(self, distance_m: f64) -> Result<(f64, f64, f64), String> {
        self.validate()?;
        if !distance_m.is_finite() || distance_m <= self.clearance_m {
            return Err("rod contact distance is at or below numerical clearance".into());
        }
        if distance_m >= self.activation_m {
            return Ok((0.0, 0.0, 0.0));
        }
        let x = distance_m / self.activation_m;
        let offset = x - 1.0;
        let log = x.ln();
        let energy = -self.stiffness_j * offset * offset * log;
        let gradient =
            -self.stiffness_j / self.activation_m * (2.0 * offset * log + offset * offset / x);
        let curvature = -self.stiffness_j / self.activation_m.powi(2)
            * (2.0 * log + 4.0 * offset / x - offset * offset / x.powi(2));
        if !energy.is_finite() || !gradient.is_finite() || !curvature.is_finite() {
            return Err("rod contact barrier overflow".into());
        }
        Ok((energy, gradient, curvature))
    }
}

impl RodModel {
    // Recomputed exhaustively at every use: no candidate cache or cap. This is
    // deliberately a small-scene CPU oracle, not the paper's GPU broad phase.
    fn for_each_contact_pair(
        &self,
        positions: &[DVec3],
        mut apply: impl FnMut([usize; 2], [usize; 2], ContactSample) -> Result<(), String>,
    ) -> Result<(), String> {
        for i in 0..self.segments.len() {
            let a = self.segments[i].vertices;
            for segment in self.segments.iter().skip(i + 1) {
                let b = segment.vertices;
                if a.iter().any(|v| b.contains(v))
                    || a.iter().any(|&x| {
                        b.iter()
                            .any(|&y| self.sewing_exclusions.contains(&unordered(x, y)))
                    })
                {
                    continue;
                }
                let [p0, p1] = a.map(|v| positions[v]);
                let [q0, q1] = b.map(|v| positions[v]);
                if (p1 - p0).length_squared() <= 1e-24 || (q1 - q0).length_squared() <= 1e-24 {
                    return Err("rod contact has a collapsed segment".into());
                }
                let (s, t) = closest_rod_segments(p0, p1, q0, q1);
                let delta = p0.lerp(p1, s) - q0.lerp(q1, t);
                let distance = delta.length();
                if !distance.is_finite() || distance <= 0.0 {
                    return Err("rod contact contains touching or non-finite segments".into());
                }
                let normal = delta / distance;
                // Any unit-axis interval gap is a lower bound on the true
                // distance, including when the closest-point solver takes its
                // nearly parallel fallback. Use relative coordinates to avoid large
                // world translations, and reserve a floating-point margin.
                let projected_a = (p1 - p0).dot(normal);
                let projected_b0 = (q0 - p0).dot(normal);
                let projected_b1 = (q1 - p0).dot(normal);
                let gap = (projected_b0.min(projected_b1) - 0.0_f64.max(projected_a))
                    .max(0.0_f64.min(projected_a) - projected_b0.max(projected_b1))
                    .max(0.0);
                let scale = (p1 - p0)
                    .length()
                    .max((q1 - q0).length())
                    .max((q0 - p0).length())
                    .max(distance);
                let safe_distance = (gap - 64.0 * f64::EPSILON * scale).max(0.0);
                apply(
                    a,
                    b,
                    ContactSample {
                        distance,
                        safe_distance,
                        normal,
                        s,
                        t,
                    },
                )?;
            }
        }
        Ok(())
    }

    fn contact_stats(
        &self,
        positions: &[DVec3],
        contact: RodContact,
    ) -> Result<(f64, Option<f64>), String> {
        let mut energy = 0.0;
        let mut minimum: Option<f64> = None;
        self.for_each_contact_pair(positions, |_, _, sample| {
            if sample.safe_distance <= contact.clearance_m {
                return Err(
                    "rod segment separation cannot be established above numerical clearance".into(),
                );
            }
            energy += contact.barrier(sample.distance)?.0;
            minimum = Some(minimum.map_or(sample.distance, |d| d.min(sample.distance)));
            Ok(())
        })?;
        if !energy.is_finite() {
            return Err("rod contact energy overflow".into());
        }
        Ok((energy, minimum))
    }

    fn local_objective(
        &self,
        vertex: usize,
        state: &RodState,
        prediction: &[DVec3],
        h: f64,
        contact: RodContact,
    ) -> Result<f64, String> {
        let mut energy = 0.5 * (state.positions[vertex] - prediction[vertex]).length_squared()
            / (self.inverse_masses[vertex] * h * h);
        for &i in &self.incident_segments[vertex] {
            let seg = &self.segments[i];
            let [a, b] = seg.vertices;
            let c = (state.positions[b] - state.positions[a]) / seg.rest_length
                - state.orientations[i] * DVec3::X;
            energy += 0.5 * seg.stretch * c.length_squared();
        }
        for &(index, _) in &self.incident_stitches[vertex] {
            let stitch = &self.stitches[index];
            energy +=
                0.5 * stitch.stiffness_n_per_m * stitch.gap(&state.positions).length_squared();
        }
        self.for_each_contact_pair(&state.positions, |a, b, sample| {
            if a.contains(&vertex) || b.contains(&vertex) {
                energy += contact.barrier(sample.distance)?.0;
            }
            Ok(())
        })?;
        if energy.is_finite() {
            Ok(energy)
        } else {
            Err("rod local objective overflow".into())
        }
    }
}

#[cfg(test)]
mod contact_tests {
    use super::*;

    #[test]
    fn projected_motion_bound_never_exceeds_sampled_segment_distance() {
        let mat = RodMaterial {
            linear_density: 1.0,
            stretch_shear_rigidity: 10.0,
            bend_twist_rigidity: 0.0,
        };
        for i in 0..80 {
            let a = i as f64 * 0.031;
            let vertices = vec![
                DVec3::new(-0.5, 0.0, 0.0),
                DVec3::new(0.5, 0.03 * a.sin(), 0.02 * a.cos()),
                DVec3::new(-0.4, 0.1 + 0.02 * a.cos(), 0.05),
                DVec3::new(0.6, 0.1 + 0.02 * a.sin(), 0.05 + 1e-7 * a),
            ];
            let model = RodModel::new(
                vertices.clone(),
                vec![[0, 1], [2, 3]],
                vec![],
                None,
                &[],
                &[],
                mat,
            )
            .unwrap();
            model
                .for_each_contact_pair(&vertices, |_, _, sample| {
                    for s in 0..=20 {
                        for t in 0..=20 {
                            let p = vertices[0].lerp(vertices[1], s as f64 / 20.0);
                            let q = vertices[2].lerp(vertices[3], t as f64 / 20.0);
                            assert!(sample.safe_distance <= (p - q).length() + 1e-14);
                        }
                    }
                    Ok(())
                })
                .unwrap();
        }
    }
}
