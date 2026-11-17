# Working instructions — ClothCraft

## Start here

Read `AI_START_HERE.md`, `ROADMAP.md` and `README.md`, then `docs/handoff.md`, `docs/architecture.md` and the documentation for the module being changed. `docs/research.md` and `docs/research-patterns.md` distinguish consulted papers, independent design choices and unimplemented methods. Do not describe cited work as available backends.

The current fitting backend is VBD + OGC: read `docs/ogc.md`. The CLI defaults to CPU; `--gpu-sim` enables hybrid computation and `--reference` selects the historical Newton solver. The workspace uses `atelier.rs`. Distinguish compute timestamps, complete step times and rendering times. OGC bounds apply from an initially separated state, with explicitly documented local sewing exclusions.

The Turbo work is documented in `docs/turbo-perf.md`. Preserve conservative pair-cache coverage, distance/bound recomputation at every detection, cooperative workgroup barriers and the worker clock independent of rendering. `scripts/turbo-perf.ps1 -UiPerf` measures computation and actual window cadence; it explicitly uses the GPU.

## Execution

The owner authorized the documented local experiments. For a new session, coordinate GPU use with the current request; past permission does not authorize all future workloads. Keep the default test suite on CPU and GPU tests opt-in. Limit small local checks to two CPU jobs/test threads.

Typical commands:

```powershell
.\scripts\check-cpu.ps1
cargo build --locked -j 2
```

The second command only compiles. The authorized GPU pass is reproduced with `scripts/first-fit.ps1 -Gpu -UiSmoke`; CPU commands are independent. Check every command result. Never add GPU tests to the default suite.

## Invariants

- The 2D coordinates in metres define material rest geometry. Never reconstruct rest geometry from the draped 3D shape.
- Fabric properties are shared; grain belongs to each panel. Changing rest geometry, density or grain requires coherent recompilation of dependent caches.
- Editable files reference panel/boundary IDs and material coordinates, never persistent vertex/triangle indices.
- Preserve `ReferenceSolver::step`, `Project` v1 and rectangular templates v1. Polygons use `GarmentTemplate` v2; fitting uses `FittingProject` v1. See `docs/first-fit.md` and `docs/meshing.md` for caches and format changes.
- Delaunay triangulation uses the meshing metric, independently of material properties. Preserve boundary constraints, check quality bounds and invalidate state caches after remeshing.
- Sewing belongs to the same implicit objective, gradient, Hessian and metrics. Do not add arbitrary position projections after Newton.
- Parameters have physical units. Document residuals, convergence, penalties and limitations; a penalty is not a nonintersection guarantee.
- Fiber guides are geometric surface attachments. They add no physical yarns, yarn contacts or measured calibration.
- Keep `model`, `energy`, `garment`, `contact` and `solver` independent of cssimpler/wgpu. Editing and rendering consume the compiled model.

## Desktop UI (cssimpler)

FC-0022 replaced the egui desktop UI with cssimpler (PR #32). The repository has no egui, eframe, egui-wgpu or egui-winit dependency; do not reintroduce one, including through optional or legacy features. Use direct wgpu for rendering/compute and plain Rust types across authoring, worker, camera and viewport interfaces. Preserve real document, history, selection and simulation behavior through native adapters.

After dependency changes, check the all-features dependency graph (`cargo tree --all-features -i egui` must find no package) and the regenerated Cargo.lock, plus CPU-only and desktop builds. New subagents default to GPT-6.1 Terra High; use GPT-6.1 Sol High for difficult problems.

## Contributions

Follow [the software factory](docs/software-factory.md). `implementation/queue.json` is the authoritative implementation queue; select work with `scripts/factory.py`, using weighted virtual deadlines and priority boosts. Agents communicate in gitignored `comms/`. Every feature uses a branch and PR, merged through `gh`. The owner authorizes commit, push, PR creation, Codex ticket/PR reviews, sensible review fixes and merge for assigned feature work without repeated permission requests. Inspect findings, fix valid issues and explain declined suggestions. Keep queue bookkeeping in the dispatcher checkout. No upstream coordinator approval is required.

Preserve user files and existing changes. Preserve license notices and attribution for reused work. Test physics changes through falsifiable properties: finite differences, objectivity, balances, convergence and limiting behavior. Update technical documentation when behavior changes.

Write code comments, UI text and documentation in English; localization is future work. Preserve serialized identifiers, existing document compatibility, source quotations and attribution. Report observed results rather than claiming a test passed because it was intended to pass.

## Editing and product research

`src/authoring.rs` owns CPU transactions and history. `src/atelier/css_runner.rs` (layout, event handling and 2D/3D tool modes) and `src/atelier/css_ui/` (property panels, rails, browsers, compositor and `studio_v2.css`) are the graphical adapters; `src/atelier/core_state.rs` holds UI-independent workspace state and `src/atelier/worker.rs` the simulation worker. Read `docs/authoring.md` before changing them. A placement preview is not new rest geometry; committing it resets dependent caches. Public manuals actually consulted are recorded in `docs/manual-study.md`. Do not advertise planned formats from `ROADMAP.md` as supported. The solver comparison contract is in `docs/solver-challenge.md`.

The 2D pattern board (drawing, hit testing, sewing and corner/fillet previews) is in `atelier/css_runner.rs`, with board rendering and input routing in `atelier/css_ui/compositor.rs` and UI-independent board navigation in `viewport_controller.rs`. Corner rounding is in `authoring/corner_tool.rs` and Bézier operations in `garment/curves.rs`. The native 3D viewport currently only orbits and zooms: the 3D rail modes (Gizmo, Grab, 3D Sew, Pin, Measure) do not yet drive interactions, and placement is shown read-only. The egui-era 3D edge sewing, grab and placement gizmo were not ported. Preserve shared boundary identities between views. `layout_2d` is excluded from physics; handles remain in local rest coordinates. Do not disable `serde_json/float_roundtrip` without checking mesh stability after reload.
