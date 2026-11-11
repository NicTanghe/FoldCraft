# Resident WGSL performance follow-up

An accompanying [paper-style report](reports/wgsl-residency/report.pdf) is available as [editable Typst](reports/wgsl-residency/report.typ), with a [bibliography, annotated sources and build notes](reports/wgsl-residency/sources.md).

FC-0045, 11 November 2026. The owner requested performance investigation after
the coupled correction removed the default T-shirt protrusion. CPU physics is
unchanged. All changes below preserve the default 16 nonlinear / 8 linear
iterations, dt, material, geometry, contact radius/stiffness and collision bounds.

## Baseline and measurement contract

On this machine, the old CPU/GPU hybrid is substantially faster than the resident
solver. Residency alone does not establish a speedup. The previous correction's
comparison to CPU omitted the much faster hybrid control.

Same default 2454-vertex / 4570-triangle T-shirt, two CPU threads, release build:

| Backend | Reset assembly, 30 steps, warm median | Draped checkpoint, 20 steps, warm median |
|---|---:|---:|
| Hybrid CPU OGC / GPU VBD | 11.04 ms | 15.94 ms |
| Resident IPC before this work | 260.18 ms | 565.30 ms |
| Resident Barrier AL before this work | 263.95 ms | 548.92 ms |

Warm medians omit the first step. The draped source is the same saved IPC
200-step checkpoint for every backend, including AL/hybrid. These comparisons
hold the input/budget fixed, but the hybrid and resident contact/search laws
are different; they do not establish equal final quality. The primary speed-only
comparison is resident before/after on identical sources and budgets. Rendering
is absent. Default-garment quality remains a separate 200-step gate.

`cloth-fit --profile-wgsl` explicitly enables GPU pass-boundary timestamp sampling.
It writes gpu-phases.json; these device measurements exclude CPU preparation,
readback and metrics and add diagnostic pass/query overhead. Production complete
step measurements use profiling off. Ordinary execution allocates no timestamp
queries. Pass-boundary markers support Apple GPUs without requiring native
inside-pass timestamps. Default test execution never initializes a GPU.

Initial IPC draped profile: roughly 320 ms PCG, 120 ms line search, 52 ms
proposal/bounds, 45 ms detection and 23 ms material gradient caching. This
identifies concrete repeat work rather than a presumed transfer bottleneck.

## Optimizations and rejected experiment

- Freeze each contact's normal, barycentric weights and positive primal curvature
  once per nonlinear sweep. Reuse in every PCG matvec; recompute after every
  detection/AL preparation. Actual trial energy and collision checks still use
  their current/trial geometry, including analytic spheres.
- Remove 64 always-zero curvature bytes from each material record (208 to 144
  bytes). Coupled curvature remains J-transpose k J; legacy Jacobi still evaluates
  its original local nonlinear curvature.
- Evaluate scalar trial energy and continuation factors without second-order AD
  arrays. Cache the frozen gradient/objective from cg_init for proposal assembly.
- Share J-dot-p once per factor per linear iteration. This experiment needs a
  separate measurement: the first profile increased PCG cost, so it must not be
  retained merely because it reduces arithmetic on paper.

Contact cache plus smaller material records reduced IPC draped GPU time from
about 570 to 438 ms, with PCG falling 320 to 184 ms. Scalar energy/proposal reuse
reduced total device time further to about 375 ms, but the shared-dot experiment
raised PCG to 207 ms. These are diagnostic samples, not final complete-step gains.

## Validation and remaining work

Eight small internal opt-in GPU tests pass, including independent f64
material/operator assembly, contact VF/EE operators for both IPC and AL,
scalar AL, full-key history retention, overflow/grab residency and stressed-patch
objective improvement. Contact cache tests poison old cache contents and require
recomputation, comparing energy, gradient and matvec to independent f64 oracles
for VF/EE, inactive IPC and retained out-of-radius AL interactions. The scalar Armijo path is also checked against f64 material energy.

Complete-step timing, full garment quality and retained/rejected variants are
recorded below. The separate
primitive barrier tangential-force defect remains open (FC-0044); performance
changes do not claim to repair it. No CPU solver implementation is changed.

The retained conservative support-axis filter reduces the draped candidate list
from about 40,500 to 9,850. A closest-point estimate chooses an axis; only the gap
between primitive support intervals can reject a pair. That gap is a lower bound
on distance for any axis, including an inaccurate near-parallel witness. The
rejection radius is r+2L+8*clearance; clearance already covers the full within-step
motion and origin-relative f32 coordinate range. The path cannot consume r of
clearance because each primitive moves by at most L. This is not a nearest-face
substitution or a reduced contact radius. AL retains unresolved historical pairs
through the existing full-key retention pass.

A temporary PCG incidence includes only positive frozen curvature. It is built
after cg_init has evaluated the complete current objective/gradient. The complete
candidate incidence is restored before local motion balls, CCD, trial energies
and AL history. Zero-curvature terms contribute exactly zero to the linear
operator; no candidate is removed from subsequent path or objective checks.
The contact cache also stores frozen distance/energy/gradient for repeated
current-iterate evaluation. It is refreshed every sweep, never reused across a
changed geometry or AL state. Final original-force audit recomputes current
geometry without using stale frozen contact values.

At the last diagnostic checkpoint, IPC draped GPU time is approximately 170 ms:
53 ms detection, 35 ms matvec, 39 ms trial energies, 23 ms material gradient
cache, 6 ms initialization and 6 ms path/proposal bounds. Vector reductions and
updates contribute about 8 ms. This is about 3.3x faster device execution than the
initial 570 ms sample, still well behind the hybrid's complete 16 ms step.

The overflow regression exposed a cache error: a failed/overflowed sweep could
read unfilled frozen distances in CCD and turn a retriable capacity overflow into
a spurious clearance error. CCD now exits on an existing error; the accepted
state remains unchanged, and pure overflow grows capacity and retries as before.

## Completed quality gate and complete-step measurements

Both resident backends passed the separate 200-step default-garment regression:

| Backend | Final maximum extension | Seam gap | RMS position difference from CPU |
|---|---:|---:|---:|
| Unchanged CPU control | 21.174% | 2.041 mm | — |
| Optimized IPC | 20.135% | 2.179 mm | 5.334 mm |
| Optimized Barrier AL | 21.318% | 2.264 mm | 4.208 mm |

All eight periodic independent f64 separation audits passed. The complete test,
including the CPU control, took 115.08 s. No geometric projection or changed
stiffness, timestep, damping, mesh or iteration count was used to get these results.

Final paired repeats, Apple M1 Pro (14 GPU cores), Metal 4, two CPU threads,
release, profiling disabled, no concurrent compilation/solver workload:

| Backend | Assembly warm median before → after (30 steps) | Draped warm median before → after (20 steps) |
|---|---:|---:|
| IPC | 245.30 → 115.04 ms (2.13x) | 547.59 → 166.36 ms (3.29x) |
| Barrier AL | 253.49 → 120.27 ms (2.11x) | 524.51 → 166.79 ms (3.14x) |
| Hybrid control | 10.34 ms | 16.13 ms |

These are **complete accepted-step times**, not shader or rendering FPS. The
first step is excluded only from warm medians; full-run means and warm P95s are
also retained in wgsl-performance-results.json. Background desktop activity and
unlocked GPU clocks remain uncontrolled. Earlier baseline/profiling samples are
retained separately rather than mixed with these repeats. No realtime or speed
advantage over the hybrid is established: it remains about 10x faster here.

The accepted trajectories are not byte-identical: f32 expression ordering and
contact insertion/reduction order change. After 20 draped steps the before/after
RMS differences are 0.087 mm (IPC) and 0.261 mm (AL). After 30 assembly steps they
are 1.315 mm and 3.815 mm, with a transient AL maximum of 41.234 mm. The complete
200-step CPU comparison above is the retained shape/stretch/sewing gate; do not
replace it with a quiet draped-state check.

The shared material-dot pass was removed after increasing measured PCG cost.
The earlier rejected CG-vector fusion remains rejected: the finer breakdown
puts all vector updates/reductions near 8 ms, much smaller than remaining contact
search, matrix products and trial energies. Those measurements justify retaining
the simpler separate passes.

## Reproduce

Build release with two jobs. Keep a baseline executable from bedc01f before
rebuilding the optimized executable. Start each case from the same source file;
never advance the source between comparisons. The paired draped source was the
saved original coupled IPC checkpoint at 200 steps; the reset source has no cached
state. Both sources' SHA256s and the baseline executable hash are in the raw data.
Local artifacts are under output/wgsl-perf; source code and summary data are tracked.

    cargo build --release --locked -j 2 --bin cloth-fit
    RAYON_NUM_THREADS=2 cloth-fit --project SOURCE.json --steps 20 --threads 2 --output CASE
    # Add --barrier-al or --hybrid-vbd for the other backends.
    # Separate diagnostic run; do not use it as a complete-step speed claim:
    RAYON_NUM_THREADS=2 cloth-fit --project SOURCE.json --profile-wgsl --steps 3 --output PROFILE
    RAYON_NUM_THREADS=2 cargo test --release --locked -j 2 --lib gpu_avbd::tests -- --ignored --test-threads=1
    RAYON_NUM_THREADS=2 cargo test --release --locked -j 2 --test gpu_avbd --test gpu_garment_quality -- --ignored --test-threads=1 --nocapture

## Remaining optimization hypotheses

Detection still refits/queries the BVH every nonlinear sweep; the hybrid refreshes
its conservative candidate cache much less often. A resident conservative cache
could cover several sweeps using an accumulated maximum-displacement envelope,
while recomputing actual distances/bounds every detection and retaining all AL
history. That needs new coverage/overflow/invalidation gates before enabling.
The current filter is not such a temporal cache.

Material factor gradients still use local AD, and matvec/trial gathers use one
invocation per vertex. Cooperative factor/vertex kernels could improve occupancy
and memory access on this small mesh. These remain hypotheses, not implemented
speedups. Simply cutting nonlinear/linear iterations could change assembly
quality; this pass did not use that shortcut. Further equal-quality work is needed
to compete with the hybrid rather than infer performance from GPU residency.

Validation also passed the six opt-in integration checks and 632 CPU-only tests.
CPU-only tests include Naga validation and never initialize a GPU. CPU solver,
energy, model, contact and OGC implementation diffs from bedc01f are empty.
