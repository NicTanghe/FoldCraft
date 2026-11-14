# Stable Cosserat rods: CPU research implementation

`cosserat` is an opt-in f64 rod-network solver, not a new cloth-shell backend or
an editor mode. It implements the material and local alternating solve from
Hsu, Wang, Wu and Yuksel, [Stable Cosserat Rods](https://graphics.cs.utah.edu/research/projects/stable-cosserat-rods/)
(SIGGRAPH 2025), with a deliberately small-scene CPU contact experiment. It
does **not** reproduce the published GPU yarn-cloth benchmarks.

The position and orientation kernels adapt the authors' MIT-licensed
[CPU implementation](https://github.com/jerry060599/StableCosseratRods/tree/11081ed22ceb3f8824f447b64557745895676d0a)
(commit `11081ed22ceb3f8824f447b64557745895676d0a`). The full original notice
and affected files are in [third-party notices](../THIRD_PARTY_NOTICES.md).
The separate YarnBall GPU code is GPL-licensed and is not included.

## Model and lifecycle

Call `RodModel::new` with *rest* vertex positions in metres, segment endpoint
pairs, joint pairs, optional rest material frames, pin lists and one material.
Each joint connects two segments sharing a vertex. Segment/joint indices are
runtime data; this API is not a garment file format. No panel, seam, saved
project or 2D material coordinate is converted into a rod implicitly. The
existing fiber guides remain geometric surface attachments, not physical yarns.

Each segment stores a unit quaternion rotating local +X onto its rest tangent.
Explicit rest frames can specify twist; otherwise a deterministic shortest-arc
frame is chosen for each rest tangent. Adjacent rest-frame differences are
compiled as rest joint quaternions. A different rest shape, density, topology,
frame or material requires a new model and compatible state. The constructor
rejects duplicate edges/joints, disconnected joints, invalid indices, isolated
vertices, invalid frames and degenerate rest lengths.

`RodState` owns positions (m), velocities (m/s), orientations, fixed-point
history and physical time. Use `initial_state` for an unstressed rest state;
retain the whole accepted state for continuation. Pinned vertices and segment
frames remain fixed during a step. State values are explicitly mutable for
experiments, so a caller changing a pinned value between steps has explicitly
changed its boundary condition. A failed step leaves the input state unchanged.

`RodMaterial` uses linear density (kg/m), isotropic stretch/shear rigidity (N)
and isotropic bend/twist rigidity (N m²). These are authored inputs, not
measured fabric calibrations. Per-segment mass is split equally between its two
endpoints; stretch coefficients are `Kss * rest_length` and joint coefficients
are `4 * Kbt / mean_rest_length`. There is no anisotropic bending or torsion
calibration. The reference +X convention is a change of material basis from the
paper's +Z, following the supplied CPU code.

## Solver and metrics

A step alternates serial Gauss–Seidel vertex block updates with the paper's
quasi-static unit-quaternion orientation updates. The default uses the
`|v| + |b|` approximation; `OrientationMethod::Refined` applies the published
one-per-sweep fixed-point update and retains gamma in the state. Angular inertia
is neglected. An implicit-Euler inertial predictor uses gravity. With contact
disabled, the initial local positions use the prior velocity; with contact
enabled, they start at the previously accepted positions so that each move can
be bounded. Unlike the upstream CPU example, this first
port omits its history-based adaptive initialization and per-segment damping.
Optional `drag_per_s` scales the final velocity by `1 - drag * dt`; settings
requiring a negative scale are rejected. A fixed sweep count is not a
convergence certificate.

`RodMetrics` reports accepted physical time and dt, complete elapsed CPU step
time in milliseconds, final elastic energy (J), maximum stretch/shear norm,
and the final free-position residual RMS (N) of the *pre-drag implicit
objective*. It does not include an orientation torque residual or certify
convergence. When contact is enabled it also reports contact energy, the
computed closest-pair distance, bounded updates and rejected local line
searches. These are CPU wall timings, not GPU timestamps or viewport cadence.

## Experimental rod self-contact

Set `RodSettings.contact` explicitly. For each nonadjacent segment pair the
solver evaluates the paper's distance barrier
`-k * (d/r - 1)^2 * ln(d/r)` for `0 < d < r`; `k` is in joules and `r` is an
activation distance in metres, **not an enforced hard rod diameter**. Normal
forces use closest-point weights; positive normal curvature is used as a local
Gauss–Newton approximation. Candidate position updates are bounded and checked
against the same local inertial, stretch and contact objective before acceptance
(with a small relative numerical tolerance). There is no post-step position projection.

Pairs are recomputed exhaustively; there is no broad phase, pair cache or
candidate limit. This small-scene implementation is expensive: a vertex update
can traverse all segment pairs repeatedly for backtracking. Pairs sharing a
vertex or locally connected by sewing are excluded entirely, including their
immediate neighborhoods, as detailed below. Starting
states must be separated above the numerical clearance for all included pairs;
initial intersections are rejected, not repaired. For the motion bound, an
interval gap projected along the candidate normal is a lower bound in exact
arithmetic and is reduced by a floating-point margin. A positive bound may be
unavailable for separated geometries, causing conservative refusal or stalls.
The rod closest-segment routine checks four endpoint candidates and the interior
candidate when the segment directions are numerically independent. Below its
near-parallel threshold the interior can be missed: energy, forces and the
reported minimum can be approximate there. Floating-point
predicates are not certified; the tests do not prove a universal nonintersection
guarantee. Friction, self-contact for excluded pairs, rod/shell or rod/avatar
contacts, and collision response for existing garment meshes are absent.

## Reproduce and validation

```sh
cargo test --locked --no-default-features -j 2 --test cosserat -- --test-threads=2
cargo test --locked --no-default-features -j 2 --lib cosserat::contact_tests -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example cosserat_probe
cargo run --locked --no-default-features -j 2 --example cosserat_weave
```

The probe adapts the authors' 10-segment cantilever topology and material values
(0.1 m segments, 1 kg/m, 100 N stretch/shear, 4 N m² bend/twist, pinned root
vertex/frame), using 16 iterations, 1/120 s steps, gravity 9.81 m/s² and drag
3 s⁻¹. One local debug-profile run after 121 steps reported 1.008333 s physical
time, tip `(0.974633068, -0.245371050, 0)` m, elastic energy 0.441212363 J,
final position residual 0.000440736 N RMS, and median/P95 complete step times
0.023792/0.029875 ms. Those times are illustrative of this tiny CPU-only
contact-free example, not a performance comparison with the paper or a quality
certificate.

Tests include a finite-difference stretch gradient, a closed-form axial implicit
step, rigid-motion and quaternion-sign invariance, free fall, twist relaxation,
80 deterministic orientation-energy cases per orientation method, residual
improvement with more sweeps, invalid-state rollback, barrier finite differences,
interior segment-force finite differences, shared-vertex exclusions, a crossing
attempt, 30 close-contact steps, one straight frictionless-sliding check and
sampled projected-bound checks. These do not
cover knits, yarn weave contacts, full cloth, production performance or every
near-degenerate geometry. GPU tests remain opt-in and no GPU workload was run.

The second probe is an original 3 × 3 crossing patch, represented by six open
rods, 30 vertices and 24 segments. Both ends of each yarn are pinned; gravity
acts for ten 2 ms steps with four iterations, 6 mm activation and an authored
0.00001 J barrier. One debug-profile run measured final computed minimum
separation 0.003843423 m, contact energy 0.000015370 J, position residual
0.001040804 N RMS, and complete-step median/P95 4.205083/5.410000 ms. This is
not a fabric or performance validation. Because each crossing is represented at
segment endpoints, several segment pairs can contribute to the same physical
crossing; the barrier is not deduplicated, and contact stiffness is dependent
on segmentation. No calibrated yarn radius or yarn-level garment attachment is
claimed.

## Experimental surface attachment

`garment::rod_attachment` provides an independent, runtime-only mapping from
panel-local material points to a caller-supplied triangulation over rod points.
The support triangles are interpolation cells; they introduce no physical
shell elements, stiffness, sewing or collisions. Binding rejects uncovered
points, degenerate cells and ambiguous overlapping cells at queried positions;
points on a shared edge are accepted only when their coefficients agree. It
does not establish that the supplied support covers the whole panel or respects
holes and boundaries. The caller must guarantee this, use support vertices
corresponding to rod positions, and rebuild the mapping whenever topology or
rest coordinates change. The mapping cannot detect reordered topology with the
same vertex count. No vertex indices are stored in authoring files.

This is a building block, not a panel-to-yarn compiler or a solver-menu option.
In particular, this mapping by itself supplies no yarn-crossing, sewing or
collision physics and is not connected to the renderer.

`GarmentRodMapping` binds the compiled vertex material coordinates of one named
panel and retains the compiled garment for identity checks. Its output is in
that panel's compiled vertex order. This makes an existing panel usable as the
visible surface, but still requires an independently constructed and correctly
ordered rod support; `evaluate` rejects a different compiled garment.

`RectRodGrid` is a first, explicitly experimental rectangular material-space
lattice generator. It places rod nodes on a regular grid, joins perpendicular
rods at shared vertices and adds bending joints along each grid line. It is a
welded lattice, **not woven yarn**: without diagonal members or crossing-angle
stiffness it has an unresisted in-plane shear mode and is not a calibrated
fabric model. Its support triangles only interpolate positions. It accepts
explicit rod parameters, grid resolution and runtime pin indices, uses a flat
2D rest shape, and does not clip to arbitrary panels, make seams, initialize
from a draped garment, handle avatar collisions or integrate with the UI.

## Experimental sewing terms

`RodModel::with_stitches` adds quadratic stitches to the same implicit
positional objective as inertia and rod stretch. A `RodStitch` has sorted,
unique runtime vertex indices and coefficients summing to zero; its gap is the
weighted position sum and its energy is `0.5 * k * |gap|²`, with `k` in N/m.
Stitch forces and their local Hessian are included in the position solve, and
the final positional residual includes stitches. `stitch_energy_j` is reported
separately from rod elastic energy. No projection is applied after the solve;
finite penalties and finite iterations do not guarantee zero gap.

`garment::rod_sewing::compile_rod_stitches` transfers the existing garment
seam quadrature to supplied garment-to-rod vertex weights. The coefficient is
`seam.stiffness * sample.weight_m`, consistent with the current cloth solver.
`GarmentRodMapping::global_weights` exposes the panel attachment weights to
construct this map. Callers must rebuild the weights and stitch model when
panel, seam or rod topology changes. This is not automatic worker/UI integration.

Stitches can be combined with experimental rod self-contact. Segment pairs are
excluded entirely when they share an endpoint or have endpoints appearing on
opposite sides of the same stitch sample. These are local sewing exclusions,
but exclude the full segment pair, not just the stitched points: separation is
not guaranteed for excluded pairs, and long segments make the excluded region
larger. Every other pair remains in detection. Sewing energy is included in the
contact line search and all positional residuals. Start from a separated state
for every nonexcluded pair. Avatar contact, friction, certified geometric
predicates and production-scale sewing convergence remain unimplemented.

`PanelRodNetwork::compile` is an additional research topology that turns each
edge of an existing compiled panel triangulation into a rod, retaining the
panel's resolution, outline and holes. Its triangles are only interpolation
support. This is a welded triangular truss, not generated woven yarn; it has no
bending joints or physical calibration and must not be treated as a cloth
backend. It starts flat from 2D material coordinates, not from a draped state.
It does not automatically apply garment pins or seams, and provides no avatar
contact. The general attachment binder's work limit can reject large panels.

`GarmentRodNetwork` is a CPU research harness that combines all compiled panel
triangle edges into one welded rod truss, uses existing compiled seam samples
and pin membership, and initializes rod positions and velocities from the
compiled garment's placed 3D state. Rod rest lengths still come solely from the
2D material coordinates. Rod rotations initialize along the placed edge
orientations; this assumes no rod bending joints. Vertex order matches the
compiled garment mesh, so the state positions can be used for that mesh. The
harness takes one explicitly supplied isotropic rod material for the entire
model; it does not convert or calibrate the garment's fabric or grain. It has
no bending joints, avatar collision, user interaction, cached-state migration or
UI/worker integration, and is not a complete garment backend.
