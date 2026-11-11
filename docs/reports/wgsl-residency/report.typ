#set document(title: "GPU Residency Is Not Enough: A Case Study of Coupled WGSL Cloth Solvers", author: "ClothCraft project", date: datetime(year: 2026, month: 11, day: 11))
#set page(paper: "a4", margin: (x: 22mm, top: 21mm, bottom: 20mm),
  header: text(size: 8pt, fill: rgb("64717c"))[ClothCraft · Technical Report CC-WGSL-2026-01],
  footer: context align(center)[#text(size: 9pt)[#counter(page).display("1")]])
#set text(font: "Libertinus Serif", size: 10.5pt, lang: "en")
#set par(justify: true, leading: .58em, spacing: .62em)
#set heading(numbering: "1.1")
#set math.equation(numbering: "(1)")
#set figure(gap: .65em)
#show link: set text(fill: rgb("245c7a"))
#show raw: set text(size: 8.3pt)
#show bibliography: it => {
  set text(size: 9.5pt)
  set par(justify: false, leading: .55em)
  it
}
#show heading.where(level: 1): set text(size: 13pt)
#show heading.where(level: 2): set text(size: 11pt)
#let data = json("../../wgsl-performance-results.json")
#let row(name) = data.rows.filter(r => r.case == name).first()
#let f(x, d: 2) = str(calc.round(x, digits: d))
#let old-ipc = row("repeat-before-ipc")
#let new-ipc = row("repeat-after-ipc")
#let old-al = row("repeat-before-al")
#let new-al = row("repeat-after-al")
#let old-ipc-d = row("repeat-before-ipc-draped")
#let new-ipc-d = row("repeat-after-ipc-draped")
#let old-al-d = row("repeat-before-al-draped")
#let new-al-d = row("repeat-after-al-draped")
#let quality = data.at("quality_200_steps")
#let tab(..cells, columns: auto) = table(columns: columns, inset: (x: 5pt, y: 5pt), stroke: .35pt + rgb("bdc7cf"), ..cells)

#align(center)[
  #v(3mm)
  #text(size: 23pt, weight: "bold")[GPU Residency Is Not Enough]
  #v(2mm)
  #text(size: 15pt)[A Case Study of Coupled WGSL Cloth Solvers]
  #v(3mm)
  #text(size: 10pt)[ClothCraft project · Engineering research report · 11 November 2026]
  #v(1mm)
  #text(size: 9pt, fill: rgb("64717c"))[Implementation snapshot: `8b034699086d8edb69e8375a649774de0d42d2d1`]
]
#v(4mm)
#block(inset: (x: 10pt, y: 9pt), fill: rgb("f0f3f5"))[
  *Abstract.* Moving an iterative cloth solve to the GPU removes repeated host–device decisions, but does not ensure faster or sufficiently converged simulation. We document two independent resident WGSL solvers: a finite-material solver with an IPC-style primitive barrier, and an experimental positive-distance-slack augmented-Lagrangian contact variant. A coupled Gauss–Newton/PCG correction first addresses a persistent default-garment shape error. We then reduce repeated contact geometry, zero-curvature traversal, material-cache traffic, and scalar energy work without reducing the 16 nonlinear / 8 linear iteration budget. On a 2,454-vertex T-shirt on an Apple M1 Pro, complete-step warm medians improve by 2.11–2.13× during assembly and 3.14–3.29× from a draped checkpoint. Both optimized solvers pass a 200-step CPU-relative shape, extension, seam, and independent separation gate. Nevertheless, their draped steps remain approximately ten times slower than the retained hybrid CPU/GPU control. These are local engineering measurements on one garment, not a reproduction of published AVBD, IPC, or BAL guarantees, or evidence of general real-time performance.
]
#v(2mm)
#text(size: 9pt)[*Keywords:* cloth simulation; WGSL; GPU residency; contact barriers; augmented Lagrangian; coupled optimization; performance validation.]

= Introduction

A common optimization hypothesis is that repeated CPU/GPU communication dominates simulation cost. The ClothCraft branch examined here initially pursued resident iteration: prediction, detection, force evaluation, acceptance, and integration execute on the GPU, with one accepted-state snapshot returned to CPU consumers per completed step. The resulting solver still exhibited a raised underarm/sleeve-root region on the bundled T-shirt, and correcting that defect increased its cost. The motivating question is therefore narrower and falsifiable: *can repeated GPU work be removed while preserving the corrected garment result?*

This report describes the implemented solver, its distinction from cited methods, the coupled correction, and a subsequent performance pass. Our contributions are engineering contributions: a reproducible diagnosis, conservative filtering and cache transformations with explicit validity conditions, and a recorded cost/quality comparison. The CPU solver, its contact implementation, and the material/rest model remain unchanged. Repository code and measurement records are the primary evidence for implementation-specific claims. @implementation @measurements

= Related work and terminology

Vertex Block Descent (VBD) solves the implicit variational problem through vertex-level block coordinate updates. Augmented VBD (AVBD) adds an augmented-Lagrangian treatment to address hard constraints and stiffness disparities. @vbd2024 @avbd2025 Our current primal search instead uses a global, inexact coupled positive-curvature solve. The UI label “WGSL AVBD” reflects the implementation's development lineage; it does not establish equivalence to published AVBD.

Incremental Potential Contact (IPC) combines a contact barrier with a nonlinear solution strategy and geometric safeguards. @ipc2020 We use an independent squared-distance primitive barrier and conservative path restrictions. We do not implement the complete IPC discretization, edge mollification, friction, or inversion protection; its published guarantees do not transfer automatically.

The published Barrier-Augmented Lagrangian (BAL) method uses adaptive augmentation sets and an inexact Newton-PCG solver. @bal2024 Our experimental alternative splits positive primitive distance into a slack variable and updates an unscaled multiplier. It is not the published BAL formulation. The retained control uses an independent VBD/Offset Geometric Contact (OGC) implementation and a different contact/search pipeline. @ogc2025 It is therefore a cost/quality control, rather than the same algorithm with extra transfers.

Geometric Contact Potential explicitly addresses discretization dependence and spurious contact forces. @gcp2025 Its relevance here is a limitation, not an implemented backend: optimizing a discrete primitive-sum barrier does not repair its separate force-law defects.

= Model and coupled solution

== Implicit objective and contact variants

Let $x^n, v^n$ denote accepted positions and velocities, $h$ the timestep, $m_i$ particle mass, $gamma$ mass-proportional damping in inverse seconds, and $g$ gravity. The resident predictor and inertial curvature are
$ hat(x)_i = x_i^n + (h v_i^n + h^2 g)/(1 + h gamma), quad a_i = m_i (1 + h gamma)/h^2. $
The primal objective for a frozen continuation stage is
$ F(x) = 1/2 sum_i a_i norm(x_i - hat(x)_i)^2 + 1/2 sum_t s_t k_t C_t (x)^2. $ <eq-objective>
Factors include membrane, bending, sewing, and attachments. Material-space 2D geometry defines rest state; it is never reconstructed from the drape. Each $k_t$ has the units required to make its term an energy. Continuation coefficients $s_t$ start at 0.1 and reach one by the final half of the default sweep budget. Their evolution changes intermediate objectives; it is not a physical material change. Pins eliminate degrees of freedom. @implementation

For primitive distance $d>0$, activation radius $r$, and contact stiffness $k_c$ in N/m, the direct barrier is
$ B(d) = cases(-k_c r^2/8 (q-1)^2 ln(q) & quad "if" 0 < d < r, 0 & quad "if" d >= r), quad q = d^2/r^2. $ <eq-barrier>
The direct-contact variant minimizes $F(x)+sum_j B(d_j (x))$. The ideal potential diverges as $d$ approaches zero. In code, finite-precision clearance checks and conservative path restrictions supplement the potential. A penalty or barrier expression alone is not an intersection certificate.

The independent slack-contact variant uses $z_j>0$, unscaled multiplier $lambda_j$ in N, and augmentation $rho_j$ in N/m:
$ cal(L)(x,z,lambda) = F(x) + sum_j [B(z_j) + lambda_j (d_j (x)-z_j) + rho_j/2 (d_j (x)-z_j)^2]. $ <eq-al>
Primal Armijo tests use the shifted nonnegative expression $B(z_j)+rho_j/2(d_j-z_j+lambda_j/rho_j)^2$ at frozen slack, dual, and augmentation. The shift is constant only within that primal solve. A bracketed 32-bisection slack minimization precedes $lambda_j <- lambda_j + rho_j(d_j-z_j)$. If $d_j=z_j$ and slack stationarity holds, then $lambda_j=B'(d_j)$, recovering the direct barrier's first-order contact force. The finite budget does not ensure those conditions.

Augmentation is a bounded inertia-based heuristic with a physical-stiffness floor. Multipliers start cold each physical timestep and persist across its nonlinear sweeps. Full primitive identities, not atomic insertion slots, carry history; unresolved historical interactions are retained even when absent from a newly detected candidate set. Slack positivity never substitutes for geometric feasibility. @implementation

== Coupled positive curvature

The initial vertex-block/Jacobi search left excessive extension and persistent garment distortion. Replacing it with the following frozen positive operator propagates membrane/sewing corrections between vertices:
$ H = A + sum_t s_t k_t g_t g_t^T + sum_j kappa_j b_j b_j^T, quad g_t = nabla C_t, quad b_j = nabla d_j, quad kappa_j = max(B''(d_j),0). $ <eq-operator>
For augmented contacts, the frozen primal curvature replaces $B''$. Analytic spheres use triangle distance and rest-area-integrated stiffness. Matrix products gather all participating vertex directions. This is a Gauss–Newton/contact search approximation; nonlinear second-factor and second-distance terms are omitted.

For any free direction $p$, $p^T H p$ is the sum of positive inertial terms and nonnegative weighted squares. Positive masses therefore make the ideal free-DOF operator positive definite. Unanchored internal factors also satisfy translation invariance: their gradients sum to zero. In contrast, treating independent per-vertex nonlinear row-norm curvature as a global material operator can resist rigid translation of a stressed patch. The translation property is tested; it does not apply to world-anchored attachments or fixed-avatar contacts. @correction

Each of 16 nonlinear sweeps performs eight preconditioned conjugate-gradient iterations. The linear solver follows the conjugate-gradient family; fixed-budget truncation is an implementation decision. @cg1952 A vertex-block preconditioner adds
$ delta_i I, quad delta_i = max(10^(-5) "tr"(D_i), 10^(-12) " N/m") $
only while inverting its blocks. This stabilizes near-singular f32 inversions without adding stiffness to @eq-objective or @eq-operator. The bounded direction must pass a global Armijo energy test; 16 available trials are recorded on device, and exhaustion retains the iterate. Neither eight PCG iterations nor 16 nonlinear sweeps certifies equilibrium.

== Resident execution and failure semantics

WGSL is the compute language; Rust/wgpu records and submits the dispatch stream. @wgsl2026 Device state includes positions, particle scratch, material factors, primitive/BVH geometry, contact records, incidence, and control flags in eight storage bindings plus a uniform binding. GPU residency here means *no per-iteration CPU solve/contact/acceptance decisions*. Scene compilation, initial intersection checks, final CPU diagnostics, and one accepted-state snapshot readback remain CPU work.

A typical sweep is: refit/search; prepare AL history if selected; cache frozen factors/contact values; initialize PCG; build temporary curvature incidence; perform PCG; restore complete incidence; bound the direction; check the path and objective; commit; update AL state; ramp continuation. Numerical failures preserve host position, velocity, and time. Pure capacity overflow grows buffers and retries the entire step. The final AL original-force audit recomputes current geometry and full-stiffness gradients rather than reusing a stale primal cache. @implementation

= Optimizations and validity conditions

== Frozen geometry and smaller records

Closest-point normals, barycentric weights, distance, energy, gradient, and nonnegative primal curvature are fixed within a PCG solve. Computing them once per nonlinear sweep avoids repeating primitive geometry in each of eight matrix products and in current-iterate evaluation. Sphere records follow the same rule. Geometry/slack/dual changes invalidate this cache. Actual trial energies still evaluate trial geometry, and the next detection refreshes distances and bounds.

Material records shrink from 208 to 144 bytes by removing four always-zero curvature vectors from the coupled path. Legacy Jacobi still computes its original local curvature. Contact records grow from 48 to 96 bytes to hold cached values: this trades storage for work and lowers the maximum pair capacity that fits a fixed buffer limit. Overflow handling remains explicit. Scalar trial energy and continuation evaluation avoid second-order AD arrays. The objective and gradient already computed during PCG initialization are reused for proposal assembly.

== Conservative support-interval rejection

The broad phase previously admitted approximately 40,500 draped IPC candidates, although only roughly 400 were active. Axis-aligned overlap is not sufficient to make a pair relevant. Let the maximum proposal displacement per primitive be $L=r/4$. Given any nonzero axis $u$, define the support-interval gap
$ ell_u = max(0, min_(a in cal(A)) u^T a - max_(b in cal(B)) u^T b, min_(b in cal(B)) u^T b - max_(a in cal(A)) u^T a)/norm(u). $ <eq-support>
Projection and Cauchy–Schwarz imply $ell_u <= d(cal(A),cal(B))$. A closest-point estimate proposes $u$; it is not itself the rejection distance. Consequently, an inaccurate near-parallel estimate can reduce culling efficiency without invalidating the exact-arithmetic lower bound.

A pair is rejected only if $ell_u > r+2L+8 epsilon$, where $epsilon$ is the solver's origin-relative numerical clearance. For any simultaneous update within the proposal envelope, primitive distance can decrease by at most $2L$. Thus a rejected pair cannot activate the radius-$r$ barrier during that update, subject to the documented numerical padding and coordinate limits. The filter reduces the recorded draped list to approximately 9,850 pairs. AL history retention still supplements the newly detected set; the filter does not discard a nonzero unresolved augmented interaction.

== Temporary positive-curvature incidence

PCG initialization first evaluates the complete frozen objective and gradient. During matrix products only, a temporary incidence contains contacts with positive frozen curvature. Contacts with exactly zero curvature contribute zero to @eq-operator. The complete candidate incidence is restored before local displacement bounds, path checks, trial energy, and AL history. This ordering matters: an inactive contact can become active at a trial position even though its current curvature is zero.

A regression caught an error-path interaction: if overflow prevented cache creation, subsequent CCD could interpret an unfilled cached distance as zero and add a spurious clearance error. CCD now exits when a prior error is set, preserving transactional retry for pure overflow. This is verified by the existing capacity-growth test.

= Experimental protocol

The recorded workload is a static-avatar T-shirt with 2,454 moving vertices and 4,570 cloth triangles. Tests use $h=1/60$ s, 16 nonlinear sweeps, eight PCG iterations, gravity 9.81 m/s², damping 1.5 s⁻¹, contact radius 3 mm, contact stiffness 100,000 N/m, and the same material/rest mesh/sewing data. The CPU and hybrid controls retain their own search/contact algorithms. The pre-optimization resident baseline is revision `bedc01f`; the optimized solver is revision `8b03469`. @measurements

Timing uses release executables, two CPU threads, an Apple M1 Pro with 14 GPU cores, Metal 4, macOS 27.0.1, and Rust `1.101.0-nightly (32dba69d6 2026-10-09)`. The pinned dependency is wgpu 30.0.1. Thirty-step assembly batches start from the same reset document. Twenty-step draped batches start from the same saved, original coupled-IPC 200-step checkpoint, including for AL and the hybrid. The source is never advanced between comparisons. No compilation or other solver workload ran concurrently with the final paired batches. Background desktop activity and unlocked clocks were not controlled.

*Measurement definitions.* A warm median/P95 uses per-call complete-step timing after omitting the first step: 29 assembly and 19 draped samples per batch. The complete-run mean includes all steps and surrounding metrics/logging work. This small difference in measurement boundary is explicit in @tab-timings. Profiling is disabled for these measurements. Pass-boundary hardware timestamps are separate diagnostic runs: they add pass/query overhead and exclude CPU preparation, readback, and gaps outside measured passes. Neither metric is rendered FPS or display cadence.

Only one final batch per backend/state/variant is recorded; there are no independent-trial confidence intervals. The quoted improvements are observed ratios on this hardware. Earlier baseline batches and cumulative profiling checkpoints remain in the data, but are not pooled into the final paired repeats. @measurements

= Results

== Complete-step cost

#figure(image("figures/complete-step-cost.svg", width: 100%), caption: [Warm complete-step medians from the final paired batches. The dashed line is the retained hybrid control, which uses a different algorithm. No rendering is measured. Source: @measurements.]) <fig-cost>

The assembly warm medians improve by approximately 2.1× and draped medians by 3.1–3.3×. The optimized draped IPC and AL medians remain 10.31× and 10.34× the hybrid median. A complete step of roughly 166–167 ms cannot sustain a 60-step/s physical clock at this timestep. Removing CPU iteration decisions has not made these methods faster than the hybrid.

#figure(
  text(size: 9.5pt, tab(columns: (1.5fr, .65fr, .95fr, .95fr, .9fr, .6fr),
    table.header([*Backend / state*], [*Variant*], [*Warm median*], [*Full mean*], [*Warm P95*], [*Gain*]),
    ..("repeat-before-ipc", "repeat-after-ipc", "repeat-before-al", "repeat-after-al", "repeat-before-ipc-draped", "repeat-after-ipc-draped", "repeat-before-al-draped", "repeat-after-al-draped", "repeat-hybrid", "repeat-hybrid-draped").map(name => {
      let r = row(name)
      let label = if name.contains("hybrid") {"Hybrid"} else if name.contains("-al") {"AL"} else {"IPC"}
      let state = if name.contains("draped") {"draped"} else {"assembly"}
      let variant = if name.contains("hybrid") {"Control"} else if name.contains("before") {"Before"} else {"After"}
      let gain = if variant == "After" {f(row(name.replace("after", "before")).warm_median_ms/r.warm_median_ms) + "×"} else {"—"}
      ([#label / #state], [#variant], [#f(r.warm_median_ms)], [#f(r.mean_complete_ms)], [#f(r.warm_p95_ms)], [#gain])
    }).flatten())),
  kind: table,
  caption: [Complete-step timing in milliseconds. Full means include all steps and metrics/logging; warm per-call medians and upper-rank P95s omit the first step. Gains compare the same resident solver before/after at fixed budget and source. The hybrid differs algorithmically. Small batches and changing assembly states limit tail interpretation. Source: @measurements.]) <tab-timings>

== Device cost and rejected variants

#figure(image("figures/phase-cost.svg", width: 100%), caption: [Diagnostic device-time decomposition. Each segment is a median of three recorded phase measurements, including the first device sample; totals are sums of phase medians, not a measured complete-step median. PCG includes initialization and vector updates/reductions. Pass instrumentation adds overhead. Source: @measurements.]) <fig-phases>

The initial draped profile spends roughly 320 ms in PCG, 120 ms in trial energy, 53 ms in proposal/bounds, 45 ms in detection, and 23 ms in material gradient caching. At the final diagnostic checkpoint, matvec is approximately 35 ms, initialization 6 ms, reductions/vector updates 8 ms, trial energy 39 ms, proposal/bounds 6 ms, and detection/incidence 53 ms. Detection becomes more expensive because of the additional filter; total work nevertheless falls because far fewer candidates reach repeated downstream evaluation. Material-gradient caching remains largely unchanged.

The sequential diagnostic checkpoints are not an isolated factorial ablation. Contact caching plus smaller material records reduced sampled device time to about 438 ms. A subsequent scalar-energy/proposal-reuse checkpoint reached about 375 ms while also containing a slower shared material-dot experiment. Filtering and active incidence followed. These cumulative snapshots cannot uniquely assign each fraction of the gain to one transformation.

Computing $J p$ once per material factor was removed after the observed PCG cost increased from about 184 to 207 ms at those checkpoints. An earlier vector-fusion experiment gave only about 3% improvement on a separate 16-vertex probe; it was also rejected. Neither result proves the transformations are universally slower. They did not provide sufficient evidence for retention in this implementation. @correction @measurements

== Garment quality and numerical validation

The unchanged CPU solver and each optimized resident solver run 200 steps from the same reset public template/avatar. Every 50 GPU steps, the independent f64 collision-world start validator audits the accepted state, yielding eight audits across the two methods. @tests

#figure(
  tab(columns: (1.15fr, 1fr, 1fr, 1.15fr),
    table.header([*Backend*], [*Max. extension*], [*Max. seam gap*], [*RMS from CPU*]),
    [CPU control], [#f(quality.cpu.extension_max_percent, d: 3)%], [#f(1000*quality.cpu.max_seam_gap_m, d: 3) mm], [—],
    [Optimized IPC], [#f(quality.ipc.extension_max_percent, d: 3)%], [#f(1000*quality.ipc.max_seam_gap_m, d: 3) mm], [#f(1000*quality.ipc.rms_difference_cpu_m, d: 3) mm],
    [Optimized AL], [#f(quality.al.extension_max_percent, d: 3)%], [#f(1000*quality.al.max_seam_gap_m, d: 3) mm], [#f(1000*quality.al.rms_difference_cpu_m, d: 3) mm]),
  caption: [Final quality after 200 steps (3.333 s simulated). Maximum extension is positive axial warp/weft extension relative to immutable material rest, not stress or force residual. RMS is over moving-vertex positions. Source: @measurements @tests.]) <tab-quality>

The fixed regression requires seam gap below 1.5 times the CPU value, maximum extension below $1.25 E_("CPU")+2$ percentage points, and RMS position difference below 12 mm. Both resident methods pass, and all eight separation audits pass. The complete test, including CPU control, takes 115.08 s. These thresholds establish this reproduction's gate, not universal physical correctness or equilibrium. Before the coupled correction, direct-barrier and AL maximum extensions were approximately 37.258% and 36.747%, respectively. @correction

Eight internal and six integration GPU checks cover f64 material/operator agreement, positive curvature and stressed-patch translation, scalar AL updates, poisoned-cache recomputation, VF/EE energy/gradient/matvec oracles, inactive IPC and retained out-of-radius AL, nearly parallel and translated support-axis cases, full-key history retention, overflow retry, grabs, pins, spheres, reciprocal VF, self-contact, and translation. The CPU-only suite reports 632 passes and includes Naga shader validation. GPU tests are opt-in; ordinary tests do not request an adapter. @implementation @tests

Trajectories are not bit-identical. In the paired runs, before/after draped RMS differences are 0.087 mm (IPC) and 0.261 mm (AL). Assembly RMS differences are 1.315 and 3.815 mm, with a transient AL maximum of 41.234 mm after 30 steps. f32 expression and atomic-insertion/reduction order change; the 200-step quality gate remains necessary. Passing this gate does not justify replacing it with a quiet-state screenshot.

= Limitations and discussion

*Model.* The primitive-sum barrier retains a separate flat-plane tangential-force defect. The coupled correction and performance transformations do not implement GCP or establish triangulation-independent frictionless contact. @gcp2025 There is no friction, inversion barrier, animated-avatar validation, arbitrary multilayer/dense-fold qualification, or calibrated-fabric claim. Local sewing exclusions remain explicit exceptions to contact coverage. The initially separated-state condition and finite-precision coordinate/clearance limits remain essential.

*Optimization.* A fixed-budget positive search operator is not the exact objective Hessian or an equilibrium certificate. AL primal/slack disagreement can change effective contact forces during incomplete convergence. Positive slacks alone do not make the cloth nonintersecting. The numerical floor regularizes the preconditioner only; it is not a physical stabilization term.

*Evidence.* One static garment, one GPU, small timing batches, unlocked clocks, uncontrolled desktop activity, and an asymmetric draped source limit generalization. The CPU quality control is independently executed, but it is a software control rather than measured real-cloth ground truth. The contact law and prediction differ between hybrid and resident methods. No statistically supported universal speedup, real-time result, or portability result follows from these measurements. This is an engineering report, not an externally peer-reviewed publication.

*Remaining cost.* Detection still refits/queries on every nonlinear sweep. A temporal candidate cache could cover several sweeps using accumulated motion envelopes, with recomputation of distances/bounds and correct AL retention at every detection. That is future work requiring coverage, invalidation, and overflow tests. Cooperative material/vertex kernels could improve occupancy and access patterns. Neither optimization is implemented by this report. Cutting iteration counts would be a separate cost/quality experiment.

= Conclusion

The measured performance problem contains substantial repeated GPU work. Conservative candidate rejection, frozen contact reuse, temporary curvature incidence, smaller factor records, and scalar energy evaluation reduce complete-step warm medians by 2.1–3.3× while retaining the default-garment quality gate. They leave the resident methods approximately ten times slower than the hybrid on the measured draped case.

= Reproducibility and source availability

The executable baseline is `bedc01f`; the optimized source is `8b03469`. Recorded JSON contains source-checkpoint SHA-256 values, a baseline executable hash, raw summaries, phase samples, and final quality measurements. @measurements Local `output/wgsl-perf` checkpoints/logs are gitignored; their complete files are not distributed with this report. An exact restart comparison therefore requires those retained local files or an explicitly regenerated, hashed common source. The public default-garment regression reconstructs its own template/avatar and is available independently. @tests

The companion `sources.md` identifies consulted primary sources, versions, citation scope, and exact internal files. `make_figures.py` derives both vector figures from the tracked JSON without launching simulation. This report imports the same JSON directly for its tables. The editable source, bibliography, figures, and compiled PDF are in `docs/reports/wgsl-residency/`. Typst 0.15.1 compiled the report. No solver source or new GPU experiment was required to prepare it.

#block(breakable: false)[
*Build the report from the repository root:*
```sh
typst compile --root . \
  docs/reports/wgsl-residency/report.typ \
  docs/reports/wgsl-residency/report.pdf
```
]

The relevant opt-in validation commands are documented in `docs/wgsl-performance.md`. Historical results and failed variants are preserved in `docs/wgsl-coupled-correction.md`; the AL design predates implementation. References below distinguish the scholarly background from this repository's own evidence.

#bibliography("references.bib", style: "ieee", title: [References])
