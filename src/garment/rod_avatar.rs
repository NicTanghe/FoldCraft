//! Experimental CPU garment-rod stepping against fixed triangle surfaces.
use super::rod_garment::GarmentRodNetwork;
use crate::{
    avatar::SurfaceMesh,
    cosserat::{RodCheckpoint, RodMetrics, RodModel, RodSettings, RodState},
    ogc::{OgcSettings, OgcWorld},
};
use std::collections::BTreeSet;

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

    pub fn model(&self) -> &RodModel {
        &self.network.model
    }
    /// Surface geometry is fixed. The state remains unchanged on failure.
    pub fn step(
        &mut self,
        state: &mut RodState,
        settings: RodSettings,
    ) -> Result<RodMetrics, String> {
        self.network.model.step_with_surface_contact(
            state,
            settings,
            &mut self.world,
            &self.contact,
        )
    }
}
