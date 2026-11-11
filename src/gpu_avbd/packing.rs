use super::*;
use crate::{
    energy_factors::{self as constraints, Kind},
    ogc::bvh::{Bounds, Bvh},
};

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Term {
    pub ids: [u32; 4],
    pub info: [u32; 4],
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
    pub gradients: [[f32; 4]; 4],
    pub curvature: [[f32; 4]; 4],
}
unsafe impl bytemuck::Zeroable for Term {}
unsafe impl bytemuck::Pod for Term {}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Node {
    lo: [f32; 4],
    hi: [f32; 4],
    info: [u32; 4],
}
unsafe impl bytemuck::Zeroable for Node {}
unsafe impl bytemuck::Pod for Node {}

pub(super) struct Packed {
    pub terms: Vec<Term>,
    pub geometry: Vec<u32>,
    pub nodes: Vec<Node>,
    pub roots: [u32; 4],
    pub offsets: [u32; 4],
    pub face_rows: u32,
    pub face_areas: u32,
    pub material_terms: usize,
}
impl Packed {
    pub fn new(solver: &VbdSolver, grab: Option<&Grab>, origin: DVec3) -> Result<Self, String> {
        let g = &solver.garment;
        let mut terms = Vec::new();
        for factor in constraints::compile(g) {
            if factor.ids.len() > 4 {
                return Err("WGSL AVBD factors require at most four vertices".into());
            }
            let mut term = Term {
                gradients: [[0.; 4]; 4],
                curvature: [[0.; 4]; 4],
                ids: [factor.ids[0] as u32; 4],
                info: [0, 0, factor.ids.len() as u32, 0],
                a: [0.; 4],
                b: [0.; 4],
                c: [factor.k as f32, 0., 0., 0.1],
            };
            for (i, v) in factor.ids.iter().enumerate() {
                term.ids[i] = *v as u32;
            }
            match factor.kind {
                Kind::Membrane(index, channel) => {
                    let tri = &g.mesh.triangles[index];
                    let m = &g.materials[tri.material];
                    let (s, c) = m.grain_degrees.to_radians().sin_cos();
                    let u = tri.inv_dm.x_axis * c + tri.inv_dm.y_axis * s;
                    let v = -tri.inv_dm.x_axis * s + tri.inv_dm.y_axis * c;
                    term.info[1] = channel as u32;
                    term.a = [(-u.x - u.y) as f32, u.x as f32, u.y as f32, 0.];
                    term.b = [(-v.x - v.y) as f32, v.x as f32, v.y as f32, 0.];
                    term.c[1] = m.compression_transition as f32;
                }
                Kind::Bend(axis) => {
                    term.info[0] = 1;
                    term.info[1] = axis as u32;
                }
                Kind::Linear(weights, axis, target) => {
                    term.info[0] = 2;
                    term.info[1] = axis as u32;
                    term.c[2] = target as f32;
                    term.b[3] = weights.iter().sum::<f64>() as f32;
                    for (i, w) in weights.iter().enumerate() {
                        term.a[i] = *w as f32;
                    }
                    // Relative coordinates preserve seam translation invariance:
                    // compensate any actual nonzero sum, never infer rest shape.
                    term.c[2] -= (weights.iter().sum::<f64>() * origin[axis]) as f32;
                }
            }
            terms.push(term);
        }
        let material_terms = terms.len();
        terms.extend(grab_terms(grab, origin));
        if terms
            .iter()
            .flat_map(|t| t.a.iter().chain(&t.b).chain(&t.c))
            .any(|v| !v.is_finite())
        {
            return Err("WGSL AVBD material/attachment data exceeds f32 range".into());
        }
        let world = &solver.world;
        let n = world.moving + world.fixed.len();
        let nt = world.topology.triangles.len();
        let mut geometry = Vec::new();
        geometry.extend((0..n).flat_map(|i| [i as u32, i as u32, i as u32, 1]));
        geometry.extend(
            world
                .topology
                .triangles
                .iter()
                .flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32, 3]),
        );
        geometry.extend(
            world
                .topology
                .edges
                .iter()
                .flat_map(|e| [e[0] as u32, e[1] as u32, e[1] as u32, 2]),
        );
        // Static tree topology is built once; WGSL refits its complete bounds.
        let x = world.positions(&g.initial_state.x);
        let mut nodes = Vec::new();
        let mut roots = [u32::MAX; 4];
        let order_offset = geometry.len() as u32;
        for (tree_index, ids) in [
            world
                .topology
                .triangles
                .iter()
                .map(|t| t.to_vec())
                .collect::<Vec<_>>(),
            world
                .topology
                .edges
                .iter()
                .map(|e| e.to_vec())
                .collect::<Vec<_>>(),
        ]
        .into_iter()
        .enumerate()
        {
            let bounds: Vec<_> = ids
                .iter()
                .map(|p| Bounds::points(p.iter().map(|v| x[*v])))
                .collect();
            let dynamic: Vec<_> = ids
                .iter()
                .map(|p| p.iter().any(|v| *v < world.moving))
                .collect();
            let tree = Bvh::new(&bounds, &dynamic);
            let base = nodes.len();
            let order_base = geometry.len() - order_offset as usize;
            let primitive_base = if tree_index == 0 { n } else { n + nt };
            geometry.extend(
                tree.primitive_order()
                    .iter()
                    .map(|i| (i + primitive_base) as u32),
            );
            for (i, root) in tree.roots().iter().enumerate() {
                roots[tree_index * 2 + i] = (base + root) as u32;
            }
            nodes.extend(tree.nodes().map(|node| Node {
                lo: (node.bounds.min - origin).as_vec3().extend(0.).to_array(),
                hi: (node.bounds.max - origin).as_vec3().extend(0.).to_array(),
                info: [
                    (order_base + node.start) as u32,
                    (node.end - node.start) as u32,
                    (base + node.escape) as u32,
                    u32::from(node.children.is_none()) | (u32::from(node.dynamic) << 1),
                ],
            }));
        }
        let exclusion_offset = geometry.len() as u32;
        let mut offset = geometry.len() + n + 1;
        for row in world.excluded_neighbors() {
            geometry.push(offset as u32);
            offset += row.len();
        }
        geometry.push(offset as u32);
        geometry.extend(
            world
                .excluded_neighbors()
                .iter()
                .flatten()
                .map(|v| *v as u32),
        );
        let mut rows = vec![Vec::new(); world.moving];
        for (i, t) in terms.iter().enumerate() {
            for &v in &t.ids[..t.info[2] as usize] {
                rows[v as usize].push(i as u32);
            }
        }
        let row_offset = geometry.len() as u32;
        offset = geometry.len() + world.moving + 1;
        for row in &rows {
            geometry.push(offset as u32);
            offset += row.len();
        }
        geometry.push(offset as u32);
        geometry.extend(rows.into_iter().flatten());
        let face_rows = geometry.len() as u32;
        let mut rows = vec![Vec::new(); world.moving];
        for (i, t) in g.mesh.triangles.iter().enumerate() {
            for v in t.vertices {
                rows[v].push(i as u32);
            }
        }
        offset = geometry.len() + world.moving + 1;
        for row in &rows {
            geometry.push(offset as u32);
            offset += row.len();
        }
        geometry.push(offset as u32);
        geometry.extend(rows.into_iter().flatten());
        let face_areas = geometry.len() as u32;
        geometry.extend(g.mesh.triangles.iter().map(|t| (t.area as f32).to_bits()));
        Ok(Self {
            terms,
            geometry,
            nodes,
            roots,
            offsets: [0, order_offset, exclusion_offset, row_offset],
            face_rows,
            face_areas,
            material_terms,
        })
    }
}

pub(super) fn grab_terms(grab: Option<&Grab>, origin: DVec3) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut add_grab = |ids: &[usize], weights: &[f64], target: DVec3, stiffness: f64| {
        for axis in 0..3 {
            let mut term = Term {
                gradients: [[0.; 4]; 4],
                curvature: [[0.; 4]; 4],
                ids: [ids[0] as u32; 4],
                info: [2, axis as u32, ids.len() as u32, 0],
                a: [0.; 4],
                b: [0., 0., 0., 1.],
                c: [stiffness as f32, 0., (target - origin)[axis] as f32, 0.1],
            };
            for (i, (&id, &w)) in ids.iter().zip(weights).enumerate() {
                term.ids[i] = id as u32;
                term.a[i] = w as f32;
            }
            terms.push(term);
        }
    };
    if let Some(grab) = grab {
        add_grab(
            &grab.vertices,
            &grab.weights,
            grab.target,
            grab.point_stiffness(),
        );
        for p in &grab.patch {
            add_grab(
                &[p.vertex],
                &[1.],
                grab.target + p.offset,
                grab.patch_stiffness() * p.weight,
            );
        }
    }
    terms
}
