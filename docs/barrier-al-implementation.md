# Experimental resident AVBD / barrier AL

FC-0036, 11 November 2026. The [design was saved first](avbd-barrier-al-design.md)
in commit 94e1542. The distance-slack variant is now implemented experimentally.
WGSL AVBD/IPC remains the launch default.

Click the solver badge in either native top bar and select
**WGSL AVBD / barrier AL (experimental)**. Fitting also supports:

    cargo run --locked -j 2 --bin cloth-fit -- --barrier-al --iterations 4 --steps 2 --output output/barrier-al-smoke

CPU-only builds reject the GPU flag. Switching pauses at displayed accepted
x/v/time, increments the worker generation, clears metrics and invalidates
caches. Press Play or Step to continue. Selecting the active solver just closes
the dropdown; Escape also closes it. This is runtime state, not a format change.

## Method and lifecycle

The implemented independent splitting is
L=F(x)+sum[B(z)+lambda(d-z)+rho(d-z)²/2], with z>0.
It combines finite-material AVBD/Jacobi position updates with slack minimization
and unscaled contact multiplier updates. This is not a reproduction of the
published BAL paper's adaptive augmentation-set/Newton-PCG formulation.

Each sweep performs one globally accepted primal update, a 32-bisection slack
solve, and a dual update. Duals start cold each physical timestep and persist
within it. The f64 scalar oracle is in src/barrier_al.rs; GPU passes are in
src/gpu_avbd/al.wgsl. Shifted nonnegative augmented energy is used only during
frozen-primal Armijo acceptance. No physical energy monotonicity across dual
updates is claimed.

The initial augmentation without a stiffness floor passed the scalar study but
gave worse coupled-cloth residual motion and a slack numerical failure.
[Rejected results are retained](avbd-barrier-al-unfloored-study.json).
The implemented rule is:

    a = 1 / sum_j [w_j² / inertial_curvature_j], over free participating vertices
    rho = max(k, clamp(sqrt(a)*sqrt(max(B''(d),0.01*a)),0.1*a,100*a))

a, rho and physical pair stiffness k are N/m. The estimate includes inertia,
not material curvature. Pins contribute no compliance; fully fixed pairs use
the direct barrier. Sphere stiffness retains rest-triangle-area integration.
Changing rho preserves unscaled lambda. The research ablation
balance_augmentation=false uses fixed rho=k.

Previous/current pair records and incident-vertex lists match the full ordered
VF or canonical EE key and primitive kind. Multipliers never follow atomic
insertion slots. Missing unresolved augmented pairs are retained before
rebuilding incidence. Release requires d>=r, |d-z|<=1e-5*r and
|lambda|<=1e-5*k*r; released pairs use the direct barrier. Method changes,
remeshing and collider replacement invalidate residency.

Safety candidates refresh every primal sweep using the original r+2L envelope,
L=r/4, numerical padding and simultaneous Lipschitz path bounds. Persistent
augmented pairs supplement safety candidates. Slacks never replace bounds on
actual positions. Overflow grows buffers and retries the whole physical step;
numerical/device errors leave host x/v/time unchanged. Eight storage bindings
are retained; only AL allocates extra history/sphere records. Iterations require
no CPU contact geometry, multiplier reads or acceptance decisions.

Slack bracketing, finite arithmetic and stationarity failures have separate
error flags. Root stationarity tolerance is
5e-5*max(k*r,|lambda|,rho*max(d,z))+1e-12 N. This protects f32 arithmetic;
it is not an equilibrium tolerance.

## Metrics and limits

Metrics include final-sweep maximum |d-z| (m), slack stationarity (N),
rho*|delta z| (N), original-objective maximum free-vertex force norm (N),
Armijo trials, and matched pairs across sweeps. The original force is a GPU
evaluation of F+B(d) at final full material stiffness. It is a maximum over
vertex force norms, not RMS or an independent f64 audit. Unavailable native
residuals are labelled unavailable.

A fixed sweep budget does not certify equilibrium. Unfinished splitting can
change effective stiffness or cause temporary attraction and poor release.
Primitive duplication, closest-feature switches, unmollified EE contact,
conservative tangential bounds, f32 clearance and local sewing exclusions are
inherited. There is no friction, inversion barrier, animated-avatar support or
dense-fold qualification.

## Optimization and observed scope

For <=64 moving vertices, a complete Armijo trial/reduction loop fits in one
workgroup. The same sixteen-trial budget, reduction order and cooperative
barriers are retained while avoiding separate trial/acceptance dispatches.
Larger meshes keep the multi-workgroup path. Uniform early exit also avoids
repeated full reductions after acceptance. Research switches retain both old
paths for ablation.

The [raw GPU study](avbd-barrier-al-gpu-study.json) uses a 16-vertex patch over a
triangle floor, 32 sweeps and eight steps per variant. First steps are excluded:

| Same solver, matching trajectory | Previous median | Optimized median | Reduction |
|---|---:|---:|---:|
| WGSL AVBD/IPC | 96.11 ms | 85.06 ms | 11.5% |
| WGSL AVBD/barrier AL | 74.78 ms | 67.45 ms | 9.8% |

These are seven warm complete-host-step samples in one local run, including
readback, excluding shader construction. They are not GPU timestamps, FPS or
dense-garment results. Inter-solver timing comparisons are not equal-quality
speedups: finite-budget residuals and trajectories differ.

Observed checks:

- CPU suite: 632 passed, one existing ignored case. GPU tests remain opt-in.
- Native library/binary CPU tests: 134 passed, five hardware cases ignored.
- Shader validation without a GPU passed.
- Hardware AL scalar energies, derivatives, slack and dual updates matched f64
  oracles at rounded inputs.
- Full-key remapping survived shuffled insertion and retained a missing unresolved
  pair. Overflow/retry and grab residency passed for both methods.
- Both methods passed freefall, pins, sphere impact, invalid-input rollback,
  reciprocal VF, self-contact, translation and independent CPU start audits.
- The Project worker switched CPU→IPC→AL→CPU→AL→IPC and stepped each backend
  without rendering. Real dropdown click dispatch/layout tests passed.
- A 466-vertex / 794-triangle fitting AL smoke accepted two four-sweep steps.
  Its final original-force maximum was 1415.97 N: startup wiring evidence,
  not equilibrium or assembled-garment qualification.

Hardware reproduction, after coordinating use:

    RAYON_NUM_THREADS=2 cargo test --locked -j 2 --lib gpu_avbd::tests -- --ignored --test-threads=1
    RAYON_NUM_THREADS=2 cargo test --locked -j 2 --test gpu_avbd -- --ignored --test-threads=1 --nocapture
    RAYON_NUM_THREADS=2 cargo test --locked -j 2 --bin marvelous-cloth wgsl_project_worker -- --ignored --test-threads=1

Desktop all-target Clippy passed with warnings denied; the existing manifest
warning about optional rfd remains. CPU-only and desktop builds succeeded.
Local review covered buffer layouts, identity matching, contact coverage,
transactional retry and stale worker results. No external Codex review was run.
