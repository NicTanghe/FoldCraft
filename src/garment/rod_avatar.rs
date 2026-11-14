//! Experimental CPU garment-rod stepping against fixed triangle surfaces.
use super::rod_garment::GarmentRodNetwork;
use crate::{
    avatar::SurfaceMesh,
    cosserat::{RodMetrics, RodModel, RodSettings, RodState},
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
