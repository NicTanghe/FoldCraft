# Resident WGSL finite AVBD / IPC-style contact

FC-0035 implements the pipeline proposed in [ClothCraft issue 242](https://github.com/erythrofr/ClothCraft/issues/242), as an **independent finite-material AVBD / Jacobi variant with a squared-distance IPC barrier**. It is the default on this branch for desktop-enabled builds: the fitting CLI and workspace, Project v1 headless and native demo, and `cloth-template simulate`. CPU-only builds retain their CPU paths and never request an adapter. No document version changes.

This combination is not a reproduction of the complete published AVBD or IPC solvers. The owner explicitly requested this branch default; that selection is not evidence of garment-scale performance or robustness. No speedup or dense-fold qualification is claimed.

## Execution and model

`src/energy_factors.rs` shares the existing scalar factorization `U = k C²/2` between the CPU laboratory and the new backend. Membrane factors retain grain, compression smoothing and material-space rest geometry; bending retains the normal-difference law; sewing retains its quadrature weights and finite physical stiffness. Pins eliminate degrees of freedom. Barycentric and brush grabs enter the same objective. No positional repair is applied after an accepted update.

`gpu_avbd/jet.wgsl` differentiates each scalar factor with second-order, three-variable automatic differentiation. Finite stiffness starts at 10% and ramps by `10 |C|`, capped at its prescribed value; a completion floor reaches the target for the final half of the budget. Budgets of one or two iterations start at the target. `C` uses the existing factor normalization: dimensionless membrane/bending, metres for linear terms (one metre normalization for ramping). Hard augmented joints and their multipliers are **not** implemented. Finite springs deliberately have no hard-constraint dual term. Local curvature uses the factor gradient outer product plus positive Hessian row-norm curvature, as in the existing finite AVBD experiment.

The update schedule is simultaneous Jacobi. Integer atomic insertion builds per-vertex contact incidence; each vertex gathers its floating-point force and local Hessian. This avoids fixed-point overflow and lost floating-point updates. Reservation order can change f32 summation order. There is no contact-dependent mesh coloring. The complete simultaneous update must pass a device-side global Armijo test; independent local descent would be insufficient for this schedule. Sixteen backtracking trials are available per sweep; exhaustion retains that iterate. A fixed budget does not certify equilibrium, and stiffness continuation changes intermediate iteration objectives.

The implicit Euler target includes full gravity and mass-proportional damping. The current accepted positions start the solve; this backend does not reuse VBD's acceleration predictor. Velocity integration, contact decisions and all iteration passes run in WGSL. Resident state survives accepted steps. Only settings and changed attachment terms upload in steady operation. Mesh/material/world replacement, changed attachment incidence, explicit reset or an external checkpoint rebuild/reinitialize the relevant resources. Worker generations explicitly reset residency. Static avatar node bounds stay resident; dynamic BVH bounds refit on every sweep.

One positions snapshot and a small status block are read back per complete step for existing CPU UI/export consumers. This is **GPU-resident iteration**, not zero transfer and not direct solver-buffer rendering. Scene compilation and initial intersection validation remain CPU work. There are no per-iteration CPU distances, movement balls, force assembly, multiplier reads or acceptance decisions.

## Contact and continuous path bound

WGSL refits and traverses stackless face and edge BVHs. Search covers both vertex/face directions, unordered edge/edge pairs, self-contact and the unchanged local sewing exclusions. Fixed/fixed pairs are excluded. Excluding a sewn quadrature neighbourhood does not exclude entire panels. No temporal candidate cache is used by this backend; the historical OGC cache and its conservative coverage are unchanged.

For unsigned finite-primitive distance `d`, activation radius `r` (m) and stiffness `k` (N/m), the implemented scalar barrier is:

```text
s = d²/r²
B(d) = -(k r²/8) (s-1)² log(s),  0 < d < r
B(d) = 0,                       d >= r
```

Energy is joules and the logarithm is dimensionless. The scalar activation is C2. Closest-feature switches and parallel edges do not imply a globally C2 contact energy. The code sums full primitive-pair barriers; it does not implement IPC Toolkit's canonical contact deduplication, edge mollification, area weighting, friction, strain limiting, inversion barriers, Newton-PCG or convergence tolerances. Duplicate closest features can increase effective stiffness and produce mesh-dependent or spurious forces. The barrier law differs from OGC; matched iteration counts are a **quality/cost comparison**, not an unchanged-physics speed comparison.

A barrier alone cannot prevent tunnelling. Each proposal has maximum per-vertex length `L = r/4`. The BVH query retains all eligible pairs within `r + 2L`, with numerical padding. A distance between convex primitives changes by at most the sum of their maximum vertex displacements. For every retained pair, the GPU restricts the common step fraction to

```text
alpha <= 0.8 (d - clearance) / (max_motion_A + max_motion_B).
```

Thus the entire simultaneous linear update retains positive clearance; checking only its end would not suffice. Unseen pairs remain outside barrier support throughout that update. Distances and bounds recompute each sweep. This conservative Lipschitz bound is not additive CCD or IPC's original CCD implementation. It may impede tangential motion and close folds. No repair of initial intersections is provided.

The Project v1 sphere is analytic: nearest triangle/sphere separation contributes to the same objective and path bound, including triangle interiors. Its radius includes half the material thickness. Fitting avatars use their actual triangle topology. The floor remains visual unless supplied as collision geometry.

All geometry is f32 in a translated frame. Clearance is `64 * f32::EPSILON` times a local coordinate envelope including the maximum within-step motion (at least 0.1 micrometre). Unresolved clearances, degenerate primitives, nonfinite local solves or invalid controls reject the complete step. This is floating-point protection, not certified exact predicates. State `x/v/time` remains unchanged on rejection. Contact-buffer overflow triggers allocation growth and a complete retry from the last accepted state; device limits produce an error, never accepted truncation or a silent switch of solvers.

## Selection, timing and validation

```sh
# New default, headless fitting (hardware GPU)
cargo run --locked -j 2 --bin cloth-fit -- --steps 2 --output output/resident
# --gpu-sim is a compatibility flag selecting that same default.

# Explicit CPU VBD / OGC, also used by CPU-only builds
cargo run --locked -j 2 --bin cloth-fit -- --cpu --steps 2 --output output/cpu-control
# Historical hybrid control and its own force/contact oracle
cargo run --locked -j 2 --bin cloth-fit -- --hybrid-vbd --check-gpu-gradients --steps 2 --output output/hybrid-control
# Historical Newton API and CLI remain available through --reference.

# No GPU initialization
RAYON_NUM_THREADS=2 cargo test --locked --no-default-features -j 2 -- --test-threads=2
# Hardware checks, opt-in after coordination
RAYON_NUM_THREADS=2 cargo test --locked -j 2 --lib gpu_avbd::tests -- --ignored --test-threads=1 --nocapture
RAYON_NUM_THREADS=2 cargo test --locked -j 2 --test gpu_avbd -- --ignored --test-threads=1
```

`step_ms`/`solve_ms` include the complete host-side step, uploads, dispatch, synchronization, retries and snapshot processing. GPU timestamp subdivision and final residual are unavailable (`null`); no rendering time or FPS is inferred. Contact counts and minimum distance describe the final sweep's detection before its update, not an independent final surface audit. The worker's optional OGC force audit explicitly refuses the different IPC objective rather than presenting another force law's residual.

The scalar CPU oracle checks barrier derivatives, C2 activation and simultaneous-motion bounds. Naga validates all WGSL entry points without a GPU. Hardware tests compare material/sewing gradients at identical rounded geometry against f64 AD, check free fall, fixed pins, transactional errors, resource replacement, sphere impact, forward/reciprocal VF, cloth self-contact and translated contact scenes. These are small regression cases; dense garments, sustained pull/release, parallel-edge sliding and matched full-step benchmark distributions still require qualification.

Sources actually consulted for this contribution: the [AVBD authors' project overview](https://graphics.cs.utah.edu/research/projects/avbd/), [IPC authors' project overview](https://ipc-sim.github.io/), and the repository's previously read paper derivations and factorization in `docs/solver-lab.md` and `docs/solver-consistency.md`. Full-resolution PDF fetching through the web reader failed due to size. No external solver implementation was copied.
