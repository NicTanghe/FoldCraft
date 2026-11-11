//! Opt-in hardware timestamps. No queries or extra synchronization by default.
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize)]
pub struct StepProfile {
    /// Sum of phase-pass device times; excludes CPU work and gaps between passes.
    pub gpu_ms: f64,
    pub phases_ms: BTreeMap<&'static str, f64>,
}

pub(super) struct Recorder {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    labels: Vec<&'static str>,
}
impl Recorder {
    pub fn new(device: &wgpu::Device, dispatch_count: usize) -> Result<Self, String> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Err("WGSL phase profiling needs TIMESTAMP_QUERY".into());
        }
        let count = dispatch_count
            .checked_mul(2)
            .and_then(|v| v.checked_add(32))
            .filter(|n| *n <= wgpu::QUERY_SET_MAX_QUERIES as usize)
            .ok_or("Too many dispatches for WGSL phase profiling")? as u32;
        let query = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("WGSL solver phases"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let buffer = |usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("WGSL solver timestamps"),
                size: count as u64 * 8,
                usage,
                mapped_at_creation: false,
            })
        };
        Ok(Self {
            query,
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            read: buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
            labels: Vec::new(),
        })
    }
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &[wgpu::ComputePipeline],
        bind: &wgpu::BindGroup,
        dispatches: &[(usize, u32)],
    ) {
        // Pass-boundary timestamps also work on Apple GPUs. Separate passes add
        // diagnostic overhead, so compare production timings with profiling off.
        for group in dispatches.chunk_by(|a, b| phase(a.0) == phase(b.0)) {
            let index = self.labels.len() as u32 * 2;
            self.labels.push(phase(group[0].0));
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(phase(group[0].0)),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &self.query,
                    beginning_of_pass_write_index: Some(index),
                    end_of_pass_write_index: Some(index + 1),
                }),
            });
            pass.set_bind_group(0, bind, &[]);
            for &(entry, count) in group {
                pass.set_pipeline(&pipelines[entry]);
                pass.dispatch_workgroups(count, 1, 1);
            }
        }
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(
            &self.query,
            0..self.labels.len() as u32 * 2,
            &self.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve,
            0,
            &self.read,
            0,
            self.labels.len() as u64 * 16,
        );
    }
    pub fn collect(&self, device: &wgpu::Device, period: f32) -> Result<StepProfile, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.read.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let data = self.read.get_mapped_range(..).map_err(|e| e.to_string())?;
        let times: &[u64] = bytemuck::cast_slice(&data);
        let mut phases_ms = BTreeMap::new();
        for (i, label) in self.labels.iter().enumerate() {
            *phases_ms.entry(*label).or_default() +=
                times[2 * i + 1].wrapping_sub(times[2 * i]) as f64 * period as f64 / 1e6;
        }
        let gpu_ms = phases_ms.values().sum();
        drop(data);
        self.read.unmap();
        Ok(StepProfile { gpu_ms, phases_ms })
    }
}

fn phase(index: usize) -> &'static str {
    match index {
        0 => "prediction",
        1 => "stiffness",
        2..=6 => "detection",
        16 | 17 => "al_prepare",
        24 => "material_cache",
        31 => "contact_cache",
        32 => "coupled_incidence",
        25 => "cg_init",
        26 => "cg_matvec",
        27 => "cg_alpha",
        28 => "cg_update",
        29 => "cg_beta",
        30 => "cg_search",
        7 | 8 | 23 => "proposal_bounds",
        9 | 10 | 22 => "line_search",
        11 => "commit",
        18 | 19 => "al_update",
        12 | 13 => "continuation",
        20 => "al_audit",
        14 => "finish",
        _ => "other",
    }
}
