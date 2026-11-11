# Sources and build notes

This accompanies [the paper-style report](report.pdf) and its editable
[Typst source](report.typ). It documents the scope of source consultation and
separates scholarly background from this project's own implementation evidence.
The report is an engineering case study, not a submitted or peer-reviewed paper.

## Build

From the repository root, with **Typst 0.15.1**:

```sh
typst compile --root . \
  docs/reports/wgsl-residency/report.typ \
  docs/reports/wgsl-residency/report.pdf
```

The `.typ` file reads `docs/wgsl-performance-results.json` directly for numerical
tables and uses the linked bibliography in [references.bib](references.bib).
The committed SVG figures are self-contained vectors with font outlines; report
compilation requires no network, Python installation, or Typst package download.
Typst's bundled Libertinus Serif supplies the body font.

To regenerate the figures separately, use a disposable Python environment with
Matplotlib 3.9.4 and NumPy 2.0.2, then run:

```sh
python docs/reports/wgsl-residency/make_figures.py
```

This script only reads recorded JSON and renders charts. It does not run a solver
or initialize a GPU. Fixed SVG hash salt and omitted timestamp metadata make the
figures reproducible with the recorded plotting versions. The phase plot sums
per-phase medians of three diagnostic samples; it does not substitute a shader
timestamp for complete-step cost. No new simulation runs were made for the report.

For PDF layout inspection, the report was compiled and every page rendered using
PyMuPDF 1.26.5. Rendering tools are temporary local tools, not repository runtime
or Rust dependencies. The compiler was downloaded from the
[official Typst v0.15.1 release](https://github.com/typst/typst/releases/tag/v0.15.1);
the archive SHA-256 matched the release asset metadata:
`48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a`.

## Scholarly and specification sources

All links below are primary sources. Metadata was checked on 11 November 2026.
The citation keys match `references.bib`. Text is independently written; no
external code, plots, or experimental results are reused as ClothCraft results.

| Key | Source and precise locator | Consultation and claim supported |
|---|---|---|
| `vbd2024` | Anka He Chen, Ziheng Liu, Yin Yang, Cem Yuksel. **Vertex Block Descent**. ACM TOG 43(4), 2024. [Authors' project/abstract](https://graphics.cs.utah.edu/research/projects/vbd/), [DOI](https://doi.org/10.1145/3658179). | Project description and bibliographic metadata checked. Supports the vertex-level block-coordinate / variational implicit-Euler background. No published performance or unconditional-stability result is asserted for the independent resident solver. |
| `avbd2025` | Chris Giles, Elie Diaz, Cem Yuksel. **Augmented Vertex Block Descent**. ACM TOG 44(4), 2025. [Authors' project](https://graphics.cs.utah.edu/research/projects/avbd/), [paper](https://graphics.cs.utah.edu/research/projects/avbd/Augmented_VBD-SIGGRAPH25.pdf), [DOI](https://doi.org/10.1145/3731195). | Project description and metadata checked; prior repository research records selected paper sections. Supports AL treatment of hard constraints and stiffness disparity. The report explicitly distinguishes its coupled finite-material solver from published AVBD. |
| `ipc2020` | Minchen Li, Zachary Ferguson, Teseo Schneider, Timothy Langlois, Denis Zorin, Daniele Panozzo, Chenfanfu Jiang, Danny M. Kaufman. **Incremental Potential Contact: Intersection- and Inversion-free Large Deformation Dynamics**. ACM TOG 39(4), article 49, 2020. [Authors' project, abstract and BibTeX](https://ipc-sim.github.io/), [DOI](https://doi.org/10.1145/3386569.3392425). | Abstract and citation checked. Supports IPC as a contact-barrier and nonlinear-solve framework. Full IPC discretization, friction, inversion protection and its guarantees are not attributed to this implementation. |
| `bal2024` | Dewen Guo, Minchen Li, Yin Yang, Sheng Li, Guoping Wang. **Barrier-Augmented Lagrangian for GPU-based Elastodynamic Contact**. ACM TOG 43(6), 2024. [Versioned paper](https://arxiv.org/html/2407.00046v1), [abstract](https://arxiv.org/abs/2407.00046v1), [DOI](https://doi.org/10.1145/3687988). | Abstract, version, section structure and publisher metadata rechecked; `docs/avbd-barrier-al-design.md` records prior consultation of §3, Algorithm 1, and §§4–5. Supports adaptive augmentation-set / Newton-PCG background. The positive-distance slack split in this report is an independent formulation. |
| `ogc2025` | Anka He Chen, Jerry Hsu, Ziheng Liu, Miles Macklin, Yin Yang, Cem Yuksel. **Offset Geometric Contact**. ACM TOG 44(4), 2025. [Authors' project](https://graphics.cs.utah.edu/research/projects/ogc/), [DOI](https://doi.org/10.1145/3731205). | Project description and metadata checked; `docs/ogc.md` records earlier selected-paper reading. Identifies the separate control's contact lineage and displacement-bound context. The authors' RTX measurements are not imported into the M1 Pro comparison. |
| `gcp2025` | Zizhou Huang, Max Paik, Zachary Ferguson, Daniele Panozzo, Denis Zorin. **Geometric Contact Potential**. ACM TOG 44(4), 2025. [arXiv v3](https://arxiv.org/abs/2402.00719v3), [DOI](https://doi.org/10.1145/3731142). | Versioned abstract and metadata checked. Supports the discussion of discretization dependence and spurious forces. The 2024 v1 had the title *Orientation-aware Incremental Potential Contact*; this report cites the revised 2025 title/version instead of mixing the two. GCP is not an implemented backend here. |
| `cg1952` | Magnus R. Hestenes, Eduard Stiefel. **Methods of Conjugate Gradients for Solving Linear Systems**. J. Res. NBS 49(6), 409–436, 1952. [NIST original issue record](https://nvlpubs.nist.gov/nistpubs/jres/049/6/cnt049-6.htm), [archived paper](https://nvlpubs.nist.gov/nistpubs/jres/049/jresv49n6p409_A1b.pdf), [NIST retrospective](https://nvlpubs.nist.gov/nistpubs/sp958-lide/html/081-085.html). | Bibliographic record and retrospective checked. Attribution for the CG family only; the complete historical paper was not re-read in this pass. Its classical convergence properties are not claimed for a fixed-budget f32 nonlinear simulation. |
| `wgsl2026` | W3C GPU for the Web Working Group. **WebGPU Shading Language**, Candidate Recommendation Draft, 6 November 2026. [Frozen specification](https://www.w3.org/TR/2026/CRD-WGSL-20261106/). | Specification identity/version checked. Names the compute language. Binding layout, synchronization and timestamp behavior in this report are evidenced by repository code, rather than inferred from the specification title. |

For VBD, AVBD, IPC, OGC, GCP and BAL, DOI/title/author-order/volume/issue metadata was also
checked against Crossref's DOI records, for example
[the BAL metadata](https://api.crossref.org/works/10.1145/3687988).
The publisher's full-text pages returned HTTP 403 in this environment, so the
open author pages and versioned preprints supply the readable primary sources.
GCP's preprint uses Max Paik while publisher metadata uses Maxwell Paik; the
bibliography uses the publisher spelling. BAL's abstract and HTML list authors
in different orders; the bibliography uses
the publisher/Crossref order shown above.

## Internal evidence and citation map

All implementation claims refer to immutable source revision
`8b034699086d8edb69e8375a649774de0d42d2d1` on `gpu_type_1`, not the moving branch.
The pre-optimization resident baseline is `bedc01f`; the raw record also stores
its executable SHA-256 and the input checkpoint SHA-256 values.

| Citation / claim | Evidence |
|---|---|
| `implementation`: resident lifecycle, allocation, host transaction / readback | [`src/gpu_avbd.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd.rs). The bibliography points to the sibling WGSL source directory. |
| Barrier and path/filter equations | [`contact.wgsl`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/contact.wgsl): `mathematical_barrier`, `ccd`, `limit_contact_motion`, `separated_beyond_envelope`. |
| Coupled operator, preconditioner, frozen cache and temporary incidence | [`coupled.wgsl`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/coupled.wgsl): `cg_init`, `cg_matvec`, `cache_contacts`, `link_coupled`. |
| Scalar trial energy, current E/g reuse, Armijo | [`solve.wgsl`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/solve.wgsl), [`jet.wgsl`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/jet.wgsl). |
| Independent AL split, history, release and slack update | [`al.wgsl`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/al.wgsl), [`barrier_al.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/barrier_al.rs), [pre-implementation design](../../avbd-barrier-al-design.md). |
| `measurements`: tables, figures, quality values, trajectory differences and software/hardware metadata | [`wgsl-performance-results.json`](../../wgsl-performance-results.json), pinned by the bibliography's GitHub permalink. Only `repeat-*` rows populate the primary timing tables/figure; `profile-*` entries supply the separate diagnostic plot. |
| `tests`: 200-step CPU-relative gate and eight separation audits | [`gpu_garment_quality.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/tests/gpu_garment_quality.rs). Fixed gates and audit interval are in source. |
| Small GPU oracles and regressions | [`gpu_avbd/tests.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/tests.rs), [`tests/gpu_avbd.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/tests/gpu_avbd.rs). |
| Definition of extension metric | [`garment/diagnostics.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/garment/diagnostics.rs): positive axial warp/weft extension, omitting compression. |
| `correction`: historical shape defect, operator motivation, rejected experiments | [`wgsl-coupled-correction.md`](../../wgsl-coupled-correction.md), [historical matched runs](../../protrusion-matched-runs.json), [rejected vector-fusion data](../../coupled-vector-fusion-study.json). |
| Timestamp measurement boundary | [`profile.rs`](https://github.com/NicTanghe/FoldCraft/blob/8b034699086d8edb69e8375a649774de0d42d2d1/src/gpu_avbd/profile.rs). Device timings exclude CPU work and gaps between measured passes. |

The CPU-only pass count, opt-in GPU check results, and complete long-test duration
are author-recorded observations in the linked technical documentation and data;
they are not an independent laboratory certification. Local raw logs/checkpoints
under `output/` and `/tmp/` were available for this work but are not distributed
by the report. Exact restart comparisons require those local files or a newly
constructed and explicitly hashed common input. The public quality test remains
reproducible without the private local output folder.

## Scope of the report change

The report adds documentation, bibliography, generated figures and a PDF only.
It does not modify CPU or GPU solver code, alter defaults, or rerun GPU workloads.
The support-axis inequality and cache validity argument describe this repository's
implementation; they are not attributed to the external papers as a new published
algorithm or an externally reviewed theorem.
