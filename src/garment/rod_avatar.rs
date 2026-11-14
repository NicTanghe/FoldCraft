//! Experimental CPU garment-rod stepping against fixed triangle surfaces.
use super::rod_garment::GarmentRodNetwork;
use crate::{
    avatar::SurfaceMesh,
    cosserat::{RodCheckpoint, RodMetrics, RodModel, RodSettings, RodState},
    ogc::{ContactSet, OgcSettings, OgcWorld},
};
use glam::DVec3;
use std::collections::BTreeSet;
use std::time::Instant;

/// Research solver owning a garment rod network and a fixed OGC surface world.
/// Rebuild this object after garment topology, sewing, or avatar changes.
pub struct GarmentRodAvatar {
    network: GarmentRodNetwork,
    world: OgcWorld,
    contact: OgcSettings,
}

/// Research checkpoint bound to the exact rod model, fixed surface and OGC
/// settings. Solver time step and other rod settings are supplied separately.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RodAvatarCheckpoint {
    version: u32,
    surface_signature: Vec<u64>,
    rod: RodCheckpoint,
}

impl GarmentRodAvatar {
    pub fn new(
        network: GarmentRodNetwork,
        avatar: &SurfaceMesh,
        state: &RodState,
        contact: OgcSettings,
    ) -> Result<Self, String> {
        contact.validate()?;
        avatar.validate()?;
        network.model.elastic_energy(state)?;
        let garment = network.garment();
        let n = garment.mesh.uv.len();
        let mut positions = state.positions.clone();
        positions.extend_from_slice(&avatar.vertices);
        let mut faces: Vec<_> = garment.mesh.triangles.iter().map(|t| t.vertices).collect();
        faces.extend(avatar.triangles.iter().map(|t| t.map(|i| i + n)));
        let mut exclusions = BTreeSet::new();
        for seam in &garment.seams {
            for sample in &seam.samples {
                for &(a, wa) in &sample.point.terms {
                    for &(b, wb) in &sample.point.terms {
                        if wa * wb < 0.0 {
                            exclusions.insert([a.min(b), a.max(b)]);
                        }
                    }
                }
            }
        }
        let mut world = OgcWorld::new(&positions, faces, n, exclusions)?;
        world.validate_start(&positions)?;
        Ok(Self {
            network,
            world,
            contact,
        })
    }
    fn surface_signature(&self) -> Vec<u64> {
        let mut signature = vec![
            self.world.moving as u64,
            self.world.fixed.len() as u64,
            self.contact.radius.to_bits(),
            self.contact.stiffness.to_bits(),
            self.contact.relaxation.to_bits(),
            self.contact.refresh_fraction.to_bits(),
        ];
        for p in &self.world.fixed {
            signature.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        }
        signature.push(self.world.topology.triangles.len() as u64);
        for face in &self.world.topology.triangles {
            signature.extend(face.map(|index| index as u64));
        }
        signature
    }

    pub fn checkpoint(&self, state: &RodState) -> Result<RodAvatarCheckpoint, String> {
        Ok(RodAvatarCheckpoint {
            version: 1,
            surface_signature: self.surface_signature(),
            rod: self.network.model.checkpoint(state)?,
        })
    }

    /// Refuses changed surface geometry, topology or OGC settings; errors leave
    /// the destination state unchanged. Detection is refreshed on the next step.
    pub fn restore_checkpoint(
        &self,
        state: &mut RodState,
        checkpoint: &RodAvatarCheckpoint,
    ) -> Result<(), String> {
        if checkpoint.version != 1 || checkpoint.surface_signature != self.surface_signature() {
            return Err("Rod avatar checkpoint does not match surface contact".into());
        }
        self.network
            .model
            .restore_checkpoint(state, &checkpoint.rod)
    }

    pub fn set_contact(&mut self, contact: OgcSettings) -> Result<(), String> {
        contact.validate()?;
        self.contact = contact;
        Ok(())
    }

    pub fn model(&self) -> &RodModel {
        &self.network.model
    }

    /// Certify the entire straight path using fresh OGC regions. Each interval
    /// remains inside one convex region, so a pair outside the current cache
    /// cannot be crossed before the next detection. Contact onset falls back
    /// to the fully coupled surface solve.
    fn certify_inactive_path(
        &mut self,
        start: &[DVec3],
        end: &[DVec3],
        mut contacts: ContactSet,
    ) -> Result<Option<(f64, f64)>, String> {
        let motion: Vec<_> = start.iter().zip(end).map(|(a, b)| *b - *a).collect();
        let mut progress = 0.0_f64;
        for _ in 0..512 {
            let remaining = 1.0 - progress;
            let mut fraction = remaining;
            for (i, delta) in motion.iter().enumerate() {
                let length = delta.length();
                if length > 0.0 {
                    fraction = fraction.min(0.8 * contacts.radii[i] / length);
                }
            }
            if !fraction.is_finite() || fraction <= 1e-14 {
                return Ok((progress > 0.0).then_some((progress, contacts.minimum_distance)));
            }
            let mut next_progress = (progress + fraction).min(1.0);
            let mut path_positions: Vec<_> = start
                .iter()
                .zip(&motion)
                .map(|(p, d)| *p + next_progress * *d)
                .collect();
            let mut inside = false;
            for _ in 0..20 {
                if path_positions
                    .iter()
                    .enumerate()
                    .all(|(i, &p)| contacts.contains(i, p, 0.0))
                {
                    inside = true;
                    break;
                }
                fraction *= 0.5;
                next_progress = progress + fraction;
                for (i, p) in path_positions.iter_mut().enumerate() {
                    *p = start[i] + next_progress * motion[i];
                }
            }
            if !inside {
                return Ok((progress > 0.0).then_some((progress, contacts.minimum_distance)));
            }
            let x = self.world.positions(&path_positions);
            let previous_distance = contacts.minimum_distance;
            contacts = self
                .world
                .detect(&x, 2.0 * self.contact.radius, &self.contact)?;
            if contacts.active_contacts > 0 || contacts.minimum_distance <= 1e-12 {
                return Ok((progress > 0.0).then_some((progress, previous_distance)));
            }
            progress = next_progress;
            if progress >= 1.0 {
                return Ok(Some((1.0, contacts.minimum_distance)));
            }
        }
        Ok((progress > 0.0).then_some((progress, contacts.minimum_distance)))
    }

    /// Surface geometry is fixed. The state remains unchanged on failure.
    pub fn step(
        &mut self,
        state: &mut RodState,
        settings: RodSettings,
    ) -> Result<RodMetrics, String> {
        let started = Instant::now();
        let x = self.world.positions(&state.positions);
        self.world.validate_start(&x)?;
        let start_contacts = self
            .world
            .detect(&x, 2.0 * self.contact.radius, &self.contact)?;
        if start_contacts.active_contacts == 0 && settings.contact.is_none() {
            let mut candidate = state.clone();
            if let Ok(mut metrics) = self.network.model.step(&mut candidate, settings)
                && let Some((progress, distance)) = self.certify_inactive_path(
                    &state.positions,
                    &candidate.positions,
                    start_contacts,
                )?
                && progress >= 0.5
            {
                if progress < 1.0 {
                    for (i, p) in candidate.positions.iter_mut().enumerate() {
                        *p = state.positions[i] + progress * (*p - state.positions[i]);
                    }
                    for (i, q) in candidate.orientations.iter_mut().enumerate() {
                        *q = state.orientations[i].slerp(*q, progress);
                        candidate.orientation_gamma[i] = state.orientation_gamma[i]
                            + progress
                                * (candidate.orientation_gamma[i] - state.orientation_gamma[i]);
                    }
                    let h = settings.dt_s;
                    for (i, v) in candidate.velocities.iter_mut().enumerate() {
                        *v = (candidate.positions[i] - state.positions[i])
                            * ((1.0 - settings.drag_per_s * h) / h);
                    }
                    metrics.elastic_energy_j = self.network.model.elastic_energy(&candidate)?;
                    metrics.stitch_energy_j = self.network.model.stitch_energy(&candidate)?;
                    metrics.max_stretch_shear = self.network.model.max_stretch_shear(&candidate)?;
                    metrics.position_residual_rms_n = None;
                    metrics.bounded_updates += 1;
                }
                let end_x = self.world.positions(&candidate.positions);
                let objective_before = self
                    .network
                    .model
                    .implicit_objective(state, state, settings)?;
                let objective_after = self
                    .network
                    .model
                    .implicit_objective(state, &candidate, settings)?;
                if objective_after <= objective_before && self.world.validate_start(&end_x).is_ok()
                {
                    metrics.surface_minimum_distance_m = distance.is_finite().then_some(distance);
                    metrics.step_ms = started.elapsed().as_secs_f64() * 1e3;
                    *state = candidate;
                    return Ok(metrics);
                }
            }
        }
        let mut metrics = self.network.model.step_with_surface_contact(
            state,
            settings,
            &mut self.world,
            &self.contact,
        )?;
        metrics.step_ms = started.elapsed().as_secs_f64() * 1e3;
        Ok(metrics)
    }
}
