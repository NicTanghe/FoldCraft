//! GPU-resident independent finite-material AVBD / IPC-style barrier backend.
//! Geometry, force assembly, path bounds, global acceptance and integration are
//! WGSL. One accepted-state readback serves CPU consumers; no iteration readback.
//! This is an independent combination, not the authors' full AVBD/IPC algorithm.
use crate::{
    garment::CompiledGarment,
    model::State,
    vbd::{Grab, VbdMetrics, VbdSettings, VbdSolver},
};
use glam::DVec3;
use std::{cell::RefCell, sync::Arc, time::Instant};
mod packing;
use packing::Packed;

pub const BACKEND: &str = "wgsl-finite-avbd-ipc";
const ENTRIES: &[&str] = &[
    "predict",
    "reset_stiffness",
    "begin_sweep",
    "refit",
    "search",
    "clear_heads",
    "link",
    "propose",
    "ccd",
    "trial",
    "accept",
    "commit",
    "ramp",
    "end_sweep",
    "finish",
    "probe_material",
];

#[repr(C)]
#[derive(Clone, Copy)]
struct Particle {
    initial: [f32; 4],
    velocity_mass: [f32; 4],
    target: [f32; 4],
    delta: [f32; 4],
    energy: [f32; 4],
}
unsafe impl bytemuck::Zeroable for Particle {}
unsafe impl bytemuck::Pod for Particle {}

struct Resident {
    garment: Arc<CompiledGarment>,
    fixed: Vec<DVec3>,
    faces: Vec<[usize; 3]>,
    world_id: u64,
    edges: Vec<[usize; 2]>,
    controls_key: Vec<u8>,
    origin: DVec3,
    packed: Packed,
    buffers: Vec<wgpu::Buffer>,
    bind: wgpu::BindGroup,
    read: wgpu::Buffer,
    expected: Option<State>,
    grab_key: Vec<u8>,
    capacity: usize,
    last_positions: Vec<[f32; 4]>,
}

pub struct GpuAvbd {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<wgpu::ComputePipeline>,
    resident: RefCell<Option<Resident>>,
    /// Analytic sphere for Project v1. The fitting pipeline uses triangle avatars.
    pub sphere: Option<crate::contact::Sphere>,
}

/// Create a compute device independently of rendering. Call only when the
/// caller has selected GPU computation; CPU tests never call this function.
pub fn request_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| {
        format!(
            "WGSL solver needs a GPU adapter: {e}. Select --cpu or --reference for CPU execution"
        )
    })?;
    if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
        return Err("WGSL default requires a hardware GPU; select --cpu for CPU execution".into());
    }
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Resident AVBD IPC compute"),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())
}
impl GpuAvbd {
    /// Reset resident resources when a workspace generation is replaced.
    pub fn reset(&self) {
        *self.resident.borrow_mut() = None;
    }
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let entries: Vec<_> = (0..9)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 8 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding == 3,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(BACKEND),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(BACKEND),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let source = [
            include_str!("gpu_avbd/main.wgsl"),
            include_str!("gpu_avbd/jet.wgsl"),
            include_str!("gpu_avbd/contact.wgsl"),
            include_str!("gpu_avbd/solve.wgsl"),
        ]
        .join("\n");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(BACKEND),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipelines = ENTRIES
            .iter()
            .map(|entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .collect();
        Self {
            device,
            queue,
            layout,
            pipelines,
            resident: RefCell::new(None),
            sphere: None,
        }
    }
    fn allocate(
        &self,
        solver: &VbdSolver,
        state: &State,
        grab: Option<&Grab>,
        capacity: usize,
    ) -> Result<Resident, String> {
        let origin = state.x[0];
        let packed = Packed::new(solver, grab, origin)?;
        let n = state.x.len();
        let total = n + solver.world.fixed.len();
        let sizes = [
            total * 16,
            n * 80,
            (packed.terms.len() * 80).max(80),
            packed.geometry.len() * 4,
            16 + capacity * 48,
            n * 4,
            (packed.nodes.len() * 48).max(48),
            32,
            128,
        ];
        let limits = self.device.limits();
        if limits.max_storage_buffers_per_shader_stage < 8
            || packed.nodes.len() > limits.max_compute_workgroups_per_dimension as usize
            || (total + solver.world.topology.edges.len()).div_ceil(64)
                > limits.max_compute_workgroups_per_dimension as usize
            || capacity.div_ceil(64) > limits.max_compute_workgroups_per_dimension as usize
            || sizes[..8].iter().any(|s| {
                (*s).max(16) > limits.max_storage_buffer_binding_size as usize
                    || *s as u64 > limits.max_buffer_size
            })
        {
            return Err(
                "WGSL AVBD scene/contact capacity exceeds device limits; state unchanged".into(),
            );
        }
        let buffers: Vec<_> = sizes
            .into_iter()
            .enumerate()
            .map(|(i, size)| {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(BACKEND),
                    size: size.max(16) as u64,
                    usage: if i == 8 {
                        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
                    } else {
                        wgpu::BufferUsages::STORAGE
                            | wgpu::BufferUsages::COPY_DST
                            | wgpu::BufferUsages::COPY_SRC
                    },
                    mapped_at_creation: false,
                })
            })
            .collect();
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(BACKEND),
            layout: &self.layout,
            entries: &entries,
        });
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AVBD accepted state snapshot"),
            size: (n * 16 + 48) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        if !packed.terms.is_empty() {
            self.queue
                .write_buffer(&buffers[2], 0, bytemuck::cast_slice(&packed.terms));
        }
        self.queue
            .write_buffer(&buffers[3], 0, bytemuck::cast_slice(&packed.geometry));
        if !packed.nodes.is_empty() {
            self.queue
                .write_buffer(&buffers[6], 0, bytemuck::cast_slice(&packed.nodes));
        }
        Ok(Resident {
            garment: solver.garment.clone(),
            fixed: solver.world.fixed.clone(),
            faces: solver.world.topology.triangles.clone(),
            world_id: solver.world.topology_id(),
            edges: solver.world.topology.edges.clone(),
            controls_key: grab_controls_key(grab),
            origin,
            packed,
            buffers,
            bind,
            read,
            expected: None,
            grab_key: grab_key(grab),
            capacity,
            last_positions: Vec::new(),
        })
    }
    pub fn step(
        &self,
        solver: &mut VbdSolver,
        state: &mut State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> Result<VbdMetrics, String> {
        let started = Instant::now();
        let mut metrics = self.step_inner(solver, state, settings, grab)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        metrics.step_ms = elapsed;
        metrics.solve_ms = elapsed;
        Ok(metrics)
    }
    fn step_inner(
        &self,
        solver: &mut VbdSolver,
        state: &mut State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> Result<VbdMetrics, String> {
        settings.validate()?;
        if [
            settings.dt,
            settings.damping,
            settings.contact.radius,
            settings.contact.stiffness,
            settings.gravity.x,
            settings.gravity.y,
            settings.gravity.z,
        ]
        .iter()
        .any(|v| !(*v as f32).is_finite())
        {
            return Err("WGSL AVBD settings exceed f32 range; state unchanged".into());
        }
        let n = solver.garment.mesh.uv.len();
        if n == 0
            || state.x.len() != n
            || state.v.len() != n
            || !state.time.is_finite()
            || !(state.time + settings.dt).is_finite()
            || state.time + settings.dt <= state.time
            || state.x.iter().chain(&state.v).any(|p| !p.is_finite())
        {
            return Err("Invalid WGSL AVBD state; state unchanged".into());
        }
        if solver.world.moving != n
            || solver.world.experimental_interior_edges
            || solver.world.experimental_primitive_distances
        {
            return Err(
                "WGSL AVBD does not support OGC research contact laws or a changed world".into(),
            );
        }
        if let Some(grab) = grab
            && (grab.vertices.iter().any(|v| *v >= n)
                || !grab.target.is_finite()
                || !grab.stiffness.is_finite()
                || grab.stiffness < 0.
                || grab.weights.iter().any(|w| !w.is_finite() || *w < 0.)
                || (grab.weights.iter().sum::<f64>() - 1.).abs() > 1e-8
                || grab.vertices[0] == grab.vertices[1]
                || grab.vertices[0] == grab.vertices[2]
                || grab.vertices[1] == grab.vertices[2]
                || !grab.valid_patch(n))
        {
            return Err("Invalid WGSL AVBD grab; state unchanged".into());
        }
        if let Some(sphere) = &self.sphere {
            sphere.validate()?;
            if !(sphere.stiffness as f32).is_finite() {
                return Err("Sphere stiffness exceeds f32 range".into());
            }
        }
        for (i, pin) in solver.garment.mesh.pins.iter().enumerate() {
            if pin.is_some_and(|p| p.distance(state.x[i]) > 1e-10) {
                return Err("WGSL AVBD pin position changed without recompilation".into());
            }
        }
        let started = Instant::now();
        let mut storage = self.resident.borrow_mut();
        let key = grab_key(grab);
        if storage.as_ref().is_none_or(|r| {
            !Arc::ptr_eq(&r.garment, &solver.garment)
                || r.fixed != solver.world.fixed
                || r.faces != solver.world.topology.triangles
                || r.world_id != solver.world.topology_id()
                || r.edges != solver.world.topology.edges
                || r.grab_key != key
        }) {
            *storage = Some(self.allocate(solver, state, grab, (n * 32).max(1024))?);
        }
        let r = storage.as_mut().unwrap();
        let controls_key = grab_controls_key(grab);
        if r.controls_key != controls_key {
            let terms = packing::grab_terms(grab, r.origin);
            if terms
                .iter()
                .flat_map(|t| t.a.iter().chain(&t.c))
                .any(|v| !v.is_finite())
            {
                return Err("WGSL attachment exceeds f32 range".into());
            }
            if !terms.is_empty() {
                self.queue.write_buffer(
                    &r.buffers[2],
                    (r.packed.material_terms * 80) as u64,
                    bytemuck::cast_slice(&terms),
                );
            }
            r.controls_key = controls_key;
        }
        loop {
            let r = storage.as_mut().unwrap();
            let all = solver.world.positions(&state.x);
            let scale = all
                .iter()
                .map(|p| (*p - r.origin).abs().max_element())
                .fold(1., f64::max);
            if scale > 1000. {
                return Err(
                    "WGSL AVBD coordinate range exceeds 1000 m; reinitialize near the cloth".into(),
                );
            }
            let scale = scale + 0.25 * settings.contact.radius * settings.iterations as f64;
            let world_scale = all.iter().map(|p| p.abs().max_element()).fold(1., f64::max);
            let clearance =
                (64. * f32::EPSILON as f64 * scale + 64. * f64::EPSILON * world_scale).max(1e-7);
            let continuing = r
                .expected
                .as_ref()
                .is_some_and(|s| s.x == state.x && s.v == state.v && s.time == state.time);
            if !continuing {
                solver.world.validate_start(&all)?;
                let positions: Vec<_> = all
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let p = (*p - r.origin).as_vec3();
                        [
                            p.x,
                            p.y,
                            p.z,
                            if i >= n || solver.garment.mesh.pins[i].is_some() {
                                1.
                            } else {
                                0.
                            },
                        ]
                    })
                    .collect();
                let particles: Vec<_> = (0..n)
                    .map(|i| {
                        let v = state.v[i].as_vec3();
                        Particle {
                            initial: positions[i],
                            velocity_mass: [v.x, v.y, v.z, solver.garment.mesh.masses[i] as f32],
                            target: [0.; 4],
                            delta: [0.; 4],
                            energy: [0.; 4],
                        }
                    })
                    .collect();
                if positions
                    .iter()
                    .flatten()
                    .chain(particles.iter().flat_map(|p| &p.velocity_mass))
                    .any(|v| !v.is_finite())
                {
                    return Err("WGSL AVBD state exceeds f32 range".into());
                }
                r.last_positions = positions[..n].to_vec();
                self.queue
                    .write_buffer(&r.buffers[0], 0, bytemuck::cast_slice(&positions));
                self.queue
                    .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&particles));
            }
            let nt = solver.world.topology.triangles.len();
            let ne = solver.world.topology.edges.len();
            let sphere = self
                .sphere
                .as_ref()
                .map(|s| {
                    let p = s.center - r.origin;
                    [p.x as f32, p.y as f32, p.z as f32, s.radius as f32]
                })
                .unwrap_or([0.; 4]);
            let mut config = Vec::<u32>::new();
            config.extend([n as u32, all.len() as u32, nt as u32, ne as u32]);
            config.extend([
                r.packed.terms.len() as u32,
                r.packed.nodes.len() as u32,
                r.capacity as u32,
                settings.iterations as u32,
            ]);
            config.extend(r.packed.roots);
            config.extend(r.packed.offsets);
            config.extend([
                r.packed.face_rows,
                solver.garment.mesh.triangles.len() as u32,
                self.sphere
                    .as_ref()
                    .map_or(0, |s| (s.stiffness as f32).to_bits()),
                r.packed.face_areas,
            ]);
            config.extend(
                [
                    settings.dt as f32,
                    settings.damping as f32,
                    settings.contact.radius as f32,
                    settings.contact.stiffness as f32,
                ]
                .map(f32::to_bits),
            );
            config.extend(
                [
                    settings.gravity.x as f32,
                    settings.gravity.y as f32,
                    settings.gravity.z as f32,
                    clearance as f32,
                ]
                .map(f32::to_bits),
            );
            config.extend(sphere.map(f32::to_bits));
            self.queue
                .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
            r.expected = None;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.clear_buffer(&r.buffers[7], 0, None);
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(BACKEND),
                    ..Default::default()
                });
                pass.set_bind_group(0, &r.bind, &[]);
                let mut dispatch = |index: usize, count: usize| {
                    if count > 0 {
                        pass.set_pipeline(&self.pipelines[index]);
                        pass.dispatch_workgroups(count as u32, 1, 1);
                    }
                };
                dispatch(0, n.div_ceil(64));
                dispatch(1, r.packed.terms.len().div_ceil(64));
                for _ in 0..settings.iterations {
                    dispatch(2, 1);
                    dispatch(3, r.packed.nodes.len());
                    dispatch(4, (all.len() + ne).div_ceil(64));
                    dispatch(5, n.div_ceil(64));
                    dispatch(6, r.capacity.div_ceil(64));
                    dispatch(7, n.div_ceil(64));
                    dispatch(
                        8,
                        r.capacity
                            .max(solver.garment.mesh.triangles.len())
                            .div_ceil(64),
                    );
                    for _ in 0..16 {
                        dispatch(9, n.div_ceil(64));
                        dispatch(10, 1);
                    }
                    dispatch(11, n.div_ceil(64));
                    dispatch(12, r.packed.terms.len().div_ceil(64));
                    dispatch(13, 1);
                }
                dispatch(14, n.div_ceil(64));
            }
            let bytes = (n * 16) as u64;
            encoder.copy_buffer_to_buffer(&r.buffers[0], 0, &r.read, 0, bytes);
            encoder.copy_buffer_to_buffer(&r.buffers[7], 0, &r.read, bytes, 32);
            encoder.copy_buffer_to_buffer(&r.buffers[4], 0, &r.read, bytes + 32, 16);
            let submission = self.queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            r.read.map_async(wgpu::MapMode::Read, .., move |result| {
                let _ = tx.send(result);
            });
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .map_err(|e| e.to_string())?;
            rx.recv()
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let data = r.read.get_mapped_range(..).map_err(|e| e.to_string())?;
            let header: &[u32] = bytemuck::cast_slice(&data[n * 16..]);
            let error = header[0];
            let contact_count = header[8] as usize;
            let active = header[5] as usize;
            let minimum = f32::from_bits(header[4]) as f64;
            if error != 0 {
                drop(data);
                r.read.unmap();
                r.expected = None;
                if error == 1 {
                    let capacity = r
                        .capacity
                        .checked_mul(2)
                        .ok_or("WGSL contact capacity overflow")?;
                    *storage = Some(self.allocate(solver, state, grab, capacity)?);
                    continue;
                }
                return Err(format!(
                    "WGSL AVBD rejected step (flags {error:#x}: overflow=1, degenerate=2, clearance=4, incidence=8, solve=16); state unchanged"
                ));
            }
            let values: &[[f32; 4]] = bytemuck::cast_slice(&data[..n * 16]);
            let raw = values.to_vec();
            let new: Vec<_> = values
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if solver.garment.mesh.pins[i].is_some() {
                        state.x[i]
                    } else {
                        DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + r.origin
                    }
                })
                .collect();
            if new.iter().any(|p| !p.is_finite()) {
                drop(data);
                r.read.unmap();
                r.expected = None;
                return Err("Non-finite WGSL AVBD output; state unchanged".into());
            }
            drop(data);
            r.read.unmap();
            for i in 0..n {
                state.v[i] = if solver.garment.mesh.pins[i].is_some() {
                    DVec3::ZERO
                } else {
                    // Mirror the resident f32 velocity exactly for checkpoint consumers.
                    let old = glam::Vec3::from_array(r.last_positions[i][..3].try_into().unwrap());
                    let p = glam::Vec3::from_array(raw[i][..3].try_into().unwrap());
                    ((p - old) / (settings.dt as f32)).as_dvec3()
                };
                state.x[i] = new[i];
            }
            r.last_positions = raw;
            state.time += settings.dt;
            r.expected = Some(state.clone());
            let extension = crate::garment::diagnostics::extension_percent(&solver.garment, state);
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            return Ok(VbdMetrics {
                time_s: state.time,
                dt_s: settings.dt,
                step_ms: elapsed,
                detection_ms: f64::NAN,
                solve_ms: elapsed,
                iterations: settings.iterations,
                contacts: active,
                contact_terms: contact_count,
                minimum_distance_m: if minimum < 1e29 {
                    minimum
                } else {
                    f64::INFINITY
                },
                detection_updates: settings.iterations,
                max_seam_gap_m: solver.garment.max_seam_gap(&state.x),
                extension_p95_percent: extension[0],
                extension_max_percent: extension[1],
                ..Default::default()
            });
        }
    }
}
fn grab_key(grab: Option<&Grab>) -> Vec<u8> {
    let Some(g) = grab else {
        return Vec::new();
    };
    let mut key = Vec::new();
    for v in g.vertices {
        key.extend(v.to_le_bytes());
    }
    for p in &g.patch {
        key.extend(p.vertex.to_le_bytes());
    }
    key
}

/// Native adapter for the historical Project v1 rest mesh. No serialized fields
/// or reference solver API change. Reconstruction follows mesh/material edits.
pub struct ProjectAvbd {
    gpu: GpuAvbd,
    cache: Option<(Arc<crate::model::Mesh>, String, VbdSolver)>,
}
impl ProjectAvbd {
    pub fn reset(&mut self) {
        self.cache = None;
        self.gpu.reset();
    }
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            gpu: GpuAvbd::new(device, queue),
            cache: None,
        }
    }
    pub fn step(
        &mut self,
        project: &crate::project::Project,
        mesh: &Arc<crate::model::Mesh>,
        state: &mut State,
    ) -> Result<VbdMetrics, String> {
        let key = serde_json::to_string(&(&project.material, &project.sphere))
            .map_err(|e| e.to_string())?;
        if self
            .cache
            .as_ref()
            .is_none_or(|(m, k, _)| !Arc::ptr_eq(m, mesh) || *k != key)
        {
            let n = mesh.uv.len();
            let garment = Arc::new(CompiledGarment {
                mesh: (**mesh).clone(),
                initial_state: state.clone(),
                arrangement_positions: state.x.clone(),
                legacy_pins: mesh.pins.clone(),
                authored_holds: vec![false; n],
                materials: vec![project.material.clone()],
                vertex_material: vec![0; n],
                seams: vec![],
                panels: vec![],
                warnings: vec![],
            });
            let solver = VbdSolver::new(garment, None, state)?;
            self.cache = Some((mesh.clone(), key, solver));
        }
        self.gpu.sphere = project.sphere.clone().map(|mut s| {
            s.radius += project.material.thickness * 0.5;
            s
        });
        let settings = VbdSettings {
            dt: project.solver.dt,
            damping: project.solver.damping,
            gravity: project.solver.gravity,
            ..Default::default()
        };
        self.gpu
            .step(&mut self.cache.as_mut().unwrap().2, state, &settings, None)
    }
}

fn grab_controls_key(grab: Option<&Grab>) -> Vec<u8> {
    let Some(g) = grab else {
        return Vec::new();
    };
    let mut key = Vec::new();
    for w in g.weights {
        key.extend(w.to_le_bytes());
    }
    for v in g.target.to_array().into_iter().chain([g.stiffness]) {
        key.extend(v.to_le_bytes());
    }
    for p in &g.patch {
        key.extend(p.vertex.to_le_bytes());
        key.extend(p.weight.to_le_bytes());
        for v in p.offset.to_array() {
            key.extend(v.to_le_bytes());
        }
    }
    key
}

#[cfg(test)]
mod tests;
