# Rust rewrite backlog

Optional planning reference inherited from upstream. Statuses and dependencies are historical snapshots, not work reservations or requirements for editing or merging in this fork. Edit this document directly as plans change. Inspect current code before treating an item as missing. Upstream links are evidence only. Administrative tasks and reservation notes have been omitted.

## EXO-UX-008

**Correct workspace format capability information**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/atelier/workspace.rs
- README.md

**Acceptance ideas**

- Describe actual supported import and export directions in the workspace and README using the existing SVG, DXF and avatar import contracts.
- Clearly distinguish generic DXF outlines, physically scaled SVG subsets and static OBJ/glTF/GLB avatars from unsupported industrial dialects, animated assets and GLB export.
- Do not change importers, solvers or docs/interoperability.md while the markings contribution is pending. Validate the rendered text against actual menu handlers and documentation.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/167


## EXO-UX-007

**Copy unheld panels from garments with authored holds**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/authoring/clipboard.rs
- tests/pattern_clipboard.rs
- docs/pins.md

**Acceptance ideas**

- Filter clipboard holds to selected panel identities; preserve selected holds so unsupported held-panel paste continues to fail explicitly.
- Unheld selections can round-trip and paste even when other panels are pinned or frozen; preserve original holds, material geometry, live positions, velocities and time.
- Reject malformed external clipboard hold references without silently dropping constraints; preserve version 1, history, sewing and legacy documents.
- Exercise pinned and frozen sources, mixed/held selections, round-trip, atomic rejection and unchanged state on CPU; do not change solver, contacts, worker or authoring.rs.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/164

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/166


## EXO-QA-006

**Ship and verify the local authoring process across platforms**

Historical status: done. Technical dependencies: EXO-UX-006.

**Starting points**

- .github/workflows/build.yml
- scripts/check-packaged-session.py
- docs/install.md

**Acceptance ideas**

- Build and stage cloth-session with the existing Linux, macOS and Windows executables; retain assets, notices and exact revision metadata.
- Exercise the staged executable in a directory containing spaces with a bounded CPU-only protocol client: verify hello/version/request IDs, explicit open of a bundled template, guarded edit, undo/redo and serialized template round-trip through a fresh process.
- Require clean EOF and exit; reject malformed replies, mismatched revisions, no-op edits and timeouts. Keep generated fixtures isolated and never overwrite user documents.
- Preserve existing platform CPU coverage and seven required check names. Do not initialize a GPU or imply live-editor attachment.
- Document artifact names, invocation and limitations. Distinguish protocol acceptance from native editor interaction and hardware/driver certification.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/151
- https://github.com/erythrofr/ClothCraft/issues/8#issuecomment-6431098836

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/150
- https://github.com/erythrofr/ClothCraft/pull/153


## EXO-CORE-001

**Agree editor, document and solver ownership contract**

Historical status: done. Technical dependencies: none.

**Starting points**

- docs/exo-contract.md

**Acceptance ideas**

- Document metres, stable panel/boundary IDs, rest/state separation and unsupported EXO fields.
- Specify which data is authoritative, revision preconditions, cache invalidation and version negotiation; include one synthetic interchange example.
- Maintain current Project v1 / GarmentTemplate v2 compatibility; no new backend selector.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_DATA_MODEL.md
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RUST_CONVERSION_STATUS_2026-10-18.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/30


## EXO-CORE-002

**Record provenance and licensing for candidate contributions**

Historical status: ready. Technical dependencies: none.

**Starting points**

- docs/exo-provenance.md

**Acceptance ideas**

- Inventory exact upstream commits and licenses for each proposed code or asset transfer.
- Separate behavior-inspired original Rust work from copied/adapted EXO code; obtain owner decisions for any incompatible transfer.
- Do not import EXO assets, implementation or binaries merely because a task cites them.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/LICENSE
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/README.md


## EXO-REV-001

**Reconcile the two notch and allowance contributions**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/garment/polygon.rs
- src/garment/markings.rs
- src/garment/shapes.rs
- src/garment/presets.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/fitting.rs
- src/atelier/board.rs
- src/atelier/authoring_ui.rs
- src/garment/export.rs
- tests/pattern_markings.rs
- tests/pattern_print_svg.rs
- src/pattern_import.rs
- src/garment/svg_import.rs
- src/garment/mod.rs
- docs/markings.md
- docs/interoperability.md

**Acceptance ideas**

- Compare PR1 and PR6 schema/semantics before choosing or reconciling them; avoid duplicate representations.
- Verify curved boundaries, round trips, mirror/split/delete behavior and metadata-only state preservation.
- Keep complete cutting contours and exported notch markers inside physical sheet bounds; cover unchanged holes and exact/near-notch splits with explicit preservation or refusal.
- Exercise native controls through the CPU named-widget harness, keeping UI draft state in authoring_ui.rs and leaving the reserved src/atelier.rs entrypoint untouched.
- Reject malformed outer/hole point and edge counts before marking helpers index them; add regressions for the public helpers and the compile path.
- Report provenance and rebase/test on current main; merge only after review.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/patternSewingNotches.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/1
- https://github.com/erythrofr/ClothCraft/pull/6


## EXO-REV-002

**Split and review darts, persistent POM and cutline proposal**

Historical status: review_existing. Technical dependencies: none.

**Starting points**

- src/garment/cutline.rs
- src/garment/curves.rs
- src/garment/domain.rs
- src/garment/seam.rs
- src/garment/polygon.rs
- src/authoring.rs
- src/authoring/geometry.rs
- tests/cutline_production.rs
- tests/persistent_pom_references.rs
- src/garment/mod.rs

**Acceptance ideas**

- Separate authoring, document migration, adapter and physics changes in PR7 before integration.
- Check cut-and-sew dart topology, boundary identity, POM migration and actual cutline units against original fixtures.
- Explicitly review src/vbd.rs and src/gpu_vbd.rs changes separately; no solver modification may enter via an authoring review unnoticed.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/nativePatternExportPreparation.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/measurements.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/pull/80


## EXO-REV-003

**Review physical SVG output contribution**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/garment/export.rs
- src/bin/cloth-template.rs
- tests/pattern_print_svg.rs
- docs/interoperability.md
- src/garment/mod.rs

**Acceptance ideas**

- Inspect PR3 physical dimensions, viewBox, layout and curve fidelity.
- Check a known 100 mm fixture in an independent SVG consumer.
- Retain preview export separately and document print/allowance limits.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/markerArtifacts.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/3
- https://github.com/erythrofr/ClothCraft/pull/77


## EXO-REV-004

**Reconcile feature map and current stable lint contributions**

Historical status: done. Technical dependencies: none.

**Starting points**

- docs/feature-map.md
- docs/manual-study.md
- tests/drafting_tools.rs

**Acceptance ideas**

- Check PR4 against the current supported compiler and avoid duplicating an already fixed lint.
- Merge useful PR5 source references into one roadmap without claiming listed methods are implemented.
- Reconcile references with this task ledger, retaining contributor attribution.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_FEATURE_ATLAS.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/4
- https://github.com/erythrofr/ClothCraft/pull/5
- https://github.com/erythrofr/ClothCraft/pull/39


## EXO-REV-005

**Extract reviewable boundaries from the EXO Studio and session proposal**

Historical status: done. Technical dependencies: none.

**Starting points**

- docs/exo-integration-review.md

**Acceptance ideas**

- Inventory PR2 components and separate web application, assets, native bridge and Rust candidates.
- Compare the exo-session portion of PR7 with the proposed versioned editor contract.
- Produce small follow-up slices; do not merge the 1868-file application snapshot as a Rust rewrite.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RUST_CONVERSION_STATUS_2026-10-18.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/2
- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/32


## EXO-CAD-001

**Implement exact numeric construction operations**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/authoring/constructions.rs
- tests/constructions.rs
- src/authoring.rs

**Acceptance ideas**

- Provide finite, unit-aware point projection, distance-along-line and line intersection operations.
- Reject degenerate/parallel inputs explicitly; test rotated and translated cases.
- Keep a pure CPU API independent of editor/model mutation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/draftingConstructions.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/26


## EXO-CAD-002

**Add numeric construction inspector actions**

Historical status: done. Technical dependencies: EXO-CAD-001.

**Starting points**

- src/authoring.rs
- src/atelier/authoring_ui.rs
- tests/drawing_commands.rs
- docs/authoring.md
- docs/handoff.md

**Acceptance ideas**

- Expose projection, perpendicular and distance actions on stable selected points.
- One validated edit creates one undo record; invalid edits leave the document unchanged.
- Verify text focus and visible units with headless UI events.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/draftingPrecision.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/38


## EXO-CAD-003

**Define topology reference migration for pattern edits**

Historical status: blocked. Technical dependencies: EXO-CORE-001, EXO-REV-001, EXO-REV-002.

**Starting points**

- src/authoring/topology_refs.rs
- tests/topology_refs.rs
- src/authoring.rs

**Acceptance ideas**

- Specify mapping of stable boundaries, partial seams, markings, POM and linked mirrors through split/merge.
- Reject ambiguous migrations rather than dropping references.
- Round-trip a curved sewn fixture and verify identity and material coordinates.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/draftingTopology.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RUST_CUTLINE_REFERENCE_PRESERVATION_20261018.md


## EXO-CAD-004

**Unfold one half-pattern across a straight fold edge**

Historical status: blocked. Technical dependencies: EXO-CAD-003.

**Starting points**

- src/authoring/unfold.rs
- src/authoring.rs
- tests/unfold.rs

**Acceptance ideas**

- Produce one valid full contour with reflected curves and material metadata.
- Handle/reject seams, marks and holes on the fold with explicit rules.
- Preserve original document on overlap/degeneracy; undo restores exact prior template.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/draftingUnfold.js


## EXO-CAD-005

**Add bounded-error freehand-to-Bezier fitting**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/garment/freehand.rs
- tests/freehand.rs
- src/garment/mod.rs

**Acceptance ideas**

- Fit finite 2D samples with cubic segments and a stated maximum error.
- Handle duplicates, corners, tiny strokes and input/output limits deterministically.
- Keep sampling in material metres; fitting alone must not create a panel.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/vectorFreehandFit.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/vectorPatternBezier.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/27


## EXO-CAD-006

**Connect freehand drawing to transactional contour editing**

Historical status: blocked. Technical dependencies: EXO-CAD-005, EXO-CAD-003.

**Starting points**

- src/atelier/board.rs
- src/authoring.rs
- tests/freehand_authoring.rs
- src/atelier.rs

**Acceptance ideas**

- Preview a stroke then commit only valid closed geometry.
- Preserve curve/boundary identity policy and explicit cancellation.
- Undo/save/reload reproduce the fitted contour.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/vectorFreehandFit.js


## EXO-CAD-007

**Edit hole contours with the existing point tools**

Historical status: blocked. Technical dependencies: EXO-CAD-003.

**Starting points**

- src/atelier/board.rs
- src/authoring.rs
- src/authoring/geometry.rs
- tests/hole_editing.rs
- src/atelier.rs

**Acceptance ideas**

- Select and edit hole anchors/curves with unambiguous contour scope.
- Reject hole/outside-boundary or hole/hole intersections atomically.
- Preserve outer sewing references and verify remeshing cache invalidation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/draftingTopology.js


## EXO-CAD-008

**Implement offset paths and cutline corner policy**

Historical status: blocked. Technical dependencies: EXO-REV-001, EXO-REV-002.

**Starting points**

- src/garment/offsets.rs
- tests/offsets.rs
- src/garment/mod.rs

**Acceptance ideas**

- Offset straight and sampled curved paths with explicit join/cap/tolerance policy.
- Handle concave corners, narrow radii, holes and self-intersections with diagnostics.
- Keep sewing/rest contours separate from manufacturing cutlines.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/patternAllowancePreview.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/nativePatternExportPreparation.js


## EXO-SEW-001

**Add a read-only sewing construction audit**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/garment/seam.rs
- src/garment/seam/audit.rs
- src/bin/cloth-template.rs
- tests/sewing_audit.rs
- docs/sewing-audit.md

**Acceptance ideas**

- Report lengths, easing intervals, dangling references and seam junction degree using stable IDs.
- Reuse compiler correspondence including partial/chained/reversed intervals.
- Distinguish authored mismatch from measured simulated seam gap; never alter the solver.
- Provide a headless CLI report with explicit units, deterministic diagnostics and documented invalid-input behavior.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/sewingConstruction.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/sewingLengthDiagnostics.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/41


## EXO-SEW-002

**Define authored trims and construction annotations**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/garment/trims.rs
- tests/trim_model.rs
- src/garment/mod.rs
- src/garment/template.rs
- src/garment/presets.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/authoring/clipboard.rs
- src/atelier/authoring_ui.rs
- src/fitting.rs
- docs/trims.md

**Acceptance ideas**

- Persist bounded, uniquely identified buttons, buttonholes, zipper paths and topstitch rows as panel-local material-space construction metadata. Validate units, geometry and panel references atomically; preserve legacy document loads.
- Implement authoring transactions with undo/redo and save/reopen. Specify and test panel duplication, deletion, clipboard, mirror, remesh and topology edits: preserve attachment identity or explicitly reject unsupported operations instead of silently dropping or misplacing trims.
- Metadata-only edits and undo/redo preserve physical mesh signature, drape and worker/solver state. Exercise actual authoring and editor-install paths and project save/reopen on CPU; distinguish these checks from a concurrent worker or native-window run.
- Treat every trim as visual/manufacturing metadata, not a physical fastener; document unsupported physical behavior. Reconcile shared files and schema semantics with the open marking and CAD contributions before implementation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/trims.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/hardwareStyleProperties.js


## EXO-SEW-003

**Add trim editing, picking and previews**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/atelier/trims_ui.rs
- src/atelier/board.rs
- src/atelier/authoring_ui.rs
- src/atelier/authoring_smoke.rs
- src/atelier/chrome.rs
- src/atelier/edges_3d.rs
- src/atelier/selection.rs
- src/render.rs
- src/garment/embedding.rs
- tests/embedding.rs
- tests/trim_authoring.rs
- src/atelier.rs
- docs/trims.md

**Acceptance ideas**

- Provide a native workflow to create, select, move, edit and delete all supported trim types, with explicit panel ownership, validation, cancellation and one undoable transaction per confirmed operation. Clearly label objects nonphysical.
- Display trim previews on the rotated/translated 2D panel and on the corresponding deformed 3D surface without changing rest geometry or simulation. Resolve material coordinates using transient attachments, invalidate them on remesh, bound work and report unsupported/out-of-panel geometry rather than silently misplace it.
- For detailed panels, investigate bounded material-space embedding and reuse the existing editor visibility acceleration instead of quadratic path rejection or repeated exhaustive rays. Preserve deterministic triangle ties, hole rejection, current-position cache invalidation and safe failure; compare accelerated results with exhaustive CPU references, and measure cost without claiming simulation or window FPS gains. Never reuse an editor visibility cache as a collision guarantee.
- Exercise real controls and commands with the named-widget CPU harness, preview geometry and cache invalidation, rollback, undo/redo and Save/Open. Respect diagnostic overlays and visibility settings; distinguish CPU tests from opt-in native-window/GPU visual acceptance.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/proceduralHardwarePreview.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/clo7NativeHardwareActions.js


## EXO-SEW-004

**Specify and validate physical zipper or button closure**

Historical status: blocked. Technical dependencies: EXO-SEW-002, EXO-PHY-001.

**Starting points**

- docs/physical-closures.md
- examples/closure_probe.rs
- tests/physical_closures.rs

**Acceptance ideas**

- Define units, contact exclusions and closure state transitions before implementation.
- Test partial opening, reopening and pull/release with original collision coverage.
- Do not claim physical closure because a zipper mesh is visible.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/backends/protocol.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/sewingConstruction.js


## EXO-SEW-005

**Define elastic seam and gathering mechanics**

Historical status: ready. Technical dependencies: EXO-SEW-001, EXO-PHY-001.

**Starting points**

- docs/elastic-seams.md
- examples/elastic_seam_probe.rs

**Acceptance ideas**

- Distinguish geometric easing from a measured elastic seam law.
- Use units and compare force/energy derivatives and unequal-length release.
- Retain current easing semantics and document unsupported combinations.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/backends/protocol.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/sewingConstruction.js


## EXO-MFG-001

**Report net material area and mass from rest geometry**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/garment/production_metrics.rs
- tests/production_metrics.rs
- src/bin/cloth-template.rs
- docs/production-metrics.md
- src/garment/mod.rs

**Acceptance ideas**

- Report per-panel and per-fabric net area, mass and counts from the compiled rest mesh.
- Check holes, multiple fabrics, rigid placements and unchanged input state.
- Expose a CPU CLI report; label it net cloth only, not cut allowance, nesting or purchasing.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/productionCost.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/measurements.js


## EXO-MFG-002

**Add cut quantities and manufacturing metadata**

Historical status: blocked. Technical dependencies: EXO-CORE-001, EXO-REV-001.

**Starting points**

- src/garment/manufacturing.rs
- tests/manufacturing_metadata.rs
- src/garment/mod.rs

**Acceptance ideas**

- Store cut quantity, fold/face/nap policy and fabric-roll assumptions with explicit defaults.
- Validate integer/range/units and preserve existing documents.
- Metadata-only edits do not reset drape or alter material mass.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/project.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/marker.js


## EXO-MFG-003

**Define sizes and point grading rules**

Historical status: blocked. Technical dependencies: EXO-CORE-001, EXO-CAD-003.

**Starting points**

- src/garment/grading.rs
- tests/grading_model.rs
- src/garment/mod.rs

**Acceptance ideas**

- Represent base size, ordered size names and stable-point displacement rules in metres.
- Reject duplicate sizes, broken references and nonfinite rules.
- Separate size variation from mesh resolution and board layout.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/grading.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/rust/exo_grading_kernel/src/lib.rs


## EXO-MFG-004

**Generate and validate a graded size run**

Historical status: blocked. Technical dependencies: EXO-MFG-003.

**Starting points**

- src/garment/grading.rs
- tests/grading_run.rs
- src/garment/mod.rs

**Acceptance ideas**

- Transform contours, curves, internal paths and dependent manufacturing references coherently.
- Preserve source garment and validate every generated size including sewn intervals.
- Report rejected sizes and geometric/seam discrepancies; no hidden fixture-specific curve repair.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/grading.js


## EXO-MFG-005

**Add grading table and size preview UI**

Historical status: blocked. Technical dependencies: EXO-MFG-004.

**Starting points**

- src/atelier/grading_ui.rs
- tests/grading_ui.rs
- src/atelier.rs

**Acceptance ideas**

- Edit rules and preview sizes with clear base/derived ownership.
- Switching preview does not overwrite the base pattern or stale simulation.
- Undo/save/reload preserves rules and selected preview policy.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/grading.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_FEATURE_ATLAS.md


## EXO-MFG-006

**Expose persistent POM measurements and size reports**

Historical status: blocked. Technical dependencies: EXO-REV-002, EXO-MFG-004.

**Starting points**

- src/atelier/measurements_ui.rs
- src/garment/measurement_reports.rs
- tests/measurement_reports.rs
- src/garment/mod.rs
- src/atelier.rs

**Acceptance ideas**

- Reuse reviewed stable measurement references; distinguish rest, draped and target dimensions.
- Export an explicit size/tolerance/unit table with missing values reported.
- Test reference migration, curve arclength and measurement updates after grading.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/measurements.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/rust/exo_measurement_kernel/src/lib.rs

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7


## EXO-MFG-007

**Define cutting-marker inputs and validity checks**

Historical status: blocked. Technical dependencies: EXO-MFG-002, EXO-CAD-008.

**Starting points**

- src/garment/marker.rs
- tests/marker_inputs.rs
- src/garment/mod.rs

**Acceptance ideas**

- Group pieces by fabric and expand cut quantities without modifying authored panels.
- Apply grain, nap, allowed rotations, roll width and clearance in metres.
- Reject unplaceable pieces and distinguish missing from placed quantities.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/marker.js


## EXO-MFG-008

**Implement a deterministic baseline cutting layout**

Historical status: blocked. Technical dependencies: EXO-MFG-007.

**Starting points**

- src/garment/marker.rs
- tests/marker_layout.rs
- src/garment/mod.rs

**Acceptance ideas**

- Generate repeatable placements with independent overlap/roll-boundary checks.
- Report consumed roll length and net/cut utilization separately.
- Preserve grain/nap restrictions; document packing quality rather than claiming optimal nesting.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/marker.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/rust/exo_marker_kernel/src/lib.rs


## EXO-MFG-009

**Add marker inspection and physical SVG export**

Historical status: blocked. Technical dependencies: EXO-MFG-008, EXO-REV-003.

**Starting points**

- src/atelier/marker_ui.rs
- src/garment/marker_export.rs
- tests/marker_export.rs
- src/garment/mod.rs
- src/atelier.rs

**Acceptance ideas**

- Inspect placed/unplaced pieces with dimensions and material grouping.
- Export physical-scale marker coordinates and a calibration ruler.
- Show constraints and failed placement reasons; rendering is not cut approval.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/markerArtifacts.js


## EXO-MFG-010

**Compute transparent fabric cost estimates**

Historical status: blocked. Technical dependencies: EXO-MFG-008.

**Starting points**

- src/garment/costing.rs
- tests/costing.rs
- src/garment/mod.rs

**Acceptance ideas**

- Use explicit currency and price per roll metre; never substitute zero for missing prices.
- Report incomplete markers and quantities instead of a false complete total.
- Export quoted CSV safely and list excluded labor/trims/shipping/taxes.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/productionCost.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/rust/exo_fabric_cost_kernel/src/lib.rs


## EXO-MFG-011

**Export a bill of materials**

Historical status: blocked. Technical dependencies: EXO-MFG-002, EXO-SEW-002.

**Starting points**

- src/garment/bom.rs
- tests/bom.rs
- src/garment/mod.rs

**Acceptance ideas**

- List fabric, piece and trim requirements with units and stable identifiers.
- Escape XML/CSV and retain missing supplier/price information explicitly.
- Cross-check quantities against the document; no guessed manufacturing certification.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/bomXml.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/factoryPackage.js


## EXO-MFG-012

**Export a revision-stamped HTML technical pack**

Historical status: blocked. Technical dependencies: EXO-MFG-006, EXO-MFG-011.

**Starting points**

- src/garment/tech_pack.rs
- tests/tech_pack.rs
- src/garment/mod.rs

**Acceptance ideas**

- Render piece specifications, construction notes, material/BOM and measurements from one document revision.
- Escape all user text and use deterministic local assets only.
- Flag unsupported/incomplete sections; validate dimensions against the source report.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/techPackHtmlExport.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/digitalTechPack.js


## EXO-MFG-013

**Add manufacturing preflight diagnostics**

Historical status: blocked. Technical dependencies: EXO-MFG-004, EXO-MFG-008, EXO-MFG-011.

**Starting points**

- src/garment/preflight.rs
- tests/preflight.rs
- src/garment/mod.rs

**Acceptance ideas**

- Report geometry, identifiers, quantities, references, grading and marker blockers with stable codes.
- A failed or missing check cannot become a success by omission.
- Keep document validity, physical fit and manufacturing approval as separate statuses.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/productionPreflight.js


## EXO-MFG-014

**Package consistent manufacturing artifacts**

Historical status: blocked. Technical dependencies: EXO-MFG-010, EXO-MFG-012, EXO-MFG-013, EXO-IO-003.

**Starting points**

- src/garment/factory_package.rs
- tests/factory_package.rs
- src/garment/mod.rs

**Acceptance ideas**

- Build a versioned manifest containing one document revision, units, file hashes and preflight results.
- Prevent path traversal/duplicate archive entries and keep output deterministic.
- Missing required outputs fail explicitly; a ZIP is not factory certification.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/factoryPackage.js


## EXO-IO-001

**Import a documented SVG pattern subset**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/garment/svg_import.rs
- tests/svg_import.rs
- src/garment/mod.rs
- Cargo.toml
- Cargo.lock
- docs/svg-import.md

**Acceptance ideas**

- Parse explicit physical units/transforms and closed contours with a stated curve tolerance.
- Reject external/script content and unsupported ambiguity; bound input resources.
- Produce unsewn editable pieces and a loss report; never infer seams from nearby paths.
- Measure curve tolerance after all transforms in metres; verify physical units, viewBox origin/aspect handling and reflected/nonuniform transforms against independent fixtures.
- Explicitly handle or reject compound holes, open contours, nested viewports and unsupported style/clipping semantics; never silently import distorted or incomplete outlines.
- Bound bytes, nesting, tokens, output points and piece count, and test invalid input rejection without external file/network resolution.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/svgPatternImport.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/101


## EXO-IO-002

**Import generic 2D DXF pattern entities**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/garment/dxf_import.rs
- tests/dxf_import.rs
- src/garment/mod.rs

**Acceptance ideas**

- Document entity subset, units, layer treatment and closure tolerance.
- Use independent line/polyline/curve fixtures and explicit unsupported-entity reports.
- Preserve separate cut/sew lines; do not advertise AAMA/ASTM compliance.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/dxf.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/88


## EXO-IO-003

**Export generic DXF cut and sewing geometry**

Historical status: blocked. Technical dependencies: EXO-REV-002, EXO-MFG-002.

**Starting points**

- src/garment/dxf_export.rs
- tests/dxf_export.rs
- src/garment/mod.rs

**Acceptance ideas**

- Reuse reviewed cutline output and write explicit units/layers/entities.
- Validate known dimensions in an independent DXF consumer.
- Distinguish generic DXF from the industrial AAMA/ASTM dialects.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/dxf.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7


## EXO-IO-004

**Add one industrial DXF dialect with independent fixtures**

Historical status: blocked. Technical dependencies: EXO-IO-002, EXO-IO-003, EXO-MFG-003.

**Starting points**

- src/garment/dxf_industrial.rs
- tests/dxf_industrial.rs
- docs/dxf-industrial.md
- src/garment/mod.rs

**Acceptance ideas**

- Choose and name AAMA or ASTM scope first, including grade rules, grain and notch encoding.
- Verify import and export separately against independent redistributable fixtures.
- Publish losses and supported entities; leave the other dialect explicitly unsupported.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_PRODUCTION_PIPELINE.md


## EXO-IO-005

**Support OBJ and MTL material-aware round trips**

Historical status: blocked. Technical dependencies: EXO-LOOK-001.

**Starting points**

- src/avatar.rs
- src/garment/obj_materials.rs
- tests/obj_materials.rs
- src/garment/mod.rs

**Acceptance ideas**

- Preserve groups, UVs, normals, material assignments and supported textures with explicit units.
- Bound and normalize referenced paths; do not fetch external resources silently.
- Check in Blender and an independent parser; retain geometry-only compatibility.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_FEATURE_ATLAS.md


## EXO-IO-006

**Export static glTF/GLB garment assets**

Historical status: blocked. Technical dependencies: EXO-LOOK-001.

**Starting points**

- src/garment/gltf_export.rs
- tests/gltf_export.rs
- src/garment/mod.rs

**Acceptance ideas**

- Export world transforms, mesh attributes, units and a documented PBR subset.
- Validate indices/material assignments and open in an independent viewer.
- Do not claim rigging, animation or editable sewing round trips.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_FEATURE_ATLAS.md


## EXO-IO-007

**Import static glTF/GLB avatars**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/gltf_avatar.rs
- tests/gltf_avatar.rs
- src/lib.rs

**Acceptance ideas**

- Apply scene transforms and units coherently to visual and collision geometry.
- Validate triangulated finite meshes and report unsupported skinning/animation.
- Use bounded asset loading and independently generated transform fixtures.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_UI_RENDERING_AND_AVATARS.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/62


## EXO-IO-008

**Produce tiled physical PDF pattern sheets**

Historical status: blocked. Technical dependencies: EXO-REV-003, EXO-MFG-002.

**Starting points**

- src/garment/pdf_export.rs
- tests/pdf_export.rs
- src/garment/mod.rs

**Acceptance ideas**

- Set physical page dimensions, overlap, registration marks and a 100 mm ruler.
- Preserve cut/sew distinction, labels and printer scaling instructions.
- Render/measure multipage fixtures; document tolerance and unsupported fonts/curves.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/markerArtifacts.js


## EXO-LOOK-001

**Integrate visual fabric appearance without resetting physics**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/garment/appearance.rs
- tests/appearance.rs
- src/garment/mod.rs
- src/garment/template.rs
- src/garment/presets.rs
- src/authoring.rs
- src/authoring/clipboard.rs
- src/fitting.rs
- src/atelier/fabrics_ui.rs
- src/atelier/authoring_ui.rs
- src/atelier/chrome.rs
- src/render.rs
- src/cloth.wgsl
- src/gpu_bench.rs
- tests/shader.rs
- docs/fabric-appearance.md

**Acceptance ideas**

- Define validated fabric color, roughness, normal/texture references and material-space scale/orientation with explicit color-space, units and asset-identity conventions. Preserve current defaults and legacy document serialization when appearance is absent.
- Integrate supported appearance into the native fabric inspector and rendered cloth, using shared fabric IDs. Preserve diagnostic overlay precedence; unsupported texture/normal assets must not be advertised as rendered. Do not stop at an unused schema.
- Appearance-only edits and Undo/Redo preserve positions, velocities, simulation time, physical cache signature and solver/worker state. Route changes through the existing transactional history/installer; appearance does not infer measured mechanics.
- Preserve appearance ownership across duplicate, clipboard, delete, material edits and portable save/reopen; explicitly reject unsupported asset operations rather than silently dropping references.
- Test invalid-input rollback, old-file defaults, exact physical-signature invariance, history/drape preservation, named inspector controls and actual renderer data preparation. Validate WGSL on CPU; record GPU/native-window acceptance separately and run it only with current authorization.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricSurfaceModel.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricPbrAppearance.js
- https://github.com/erythrofr/ClothCraft/issues/126

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/128


## EXO-LOOK-002

**Render the basic fabric appearance model**

Historical status: blocked. Technical dependencies: EXO-LOOK-001.

**Starting points**

- src/render.rs
- src/render/materials.rs
- tests/material_render_data.rs

**Acceptance ideas**

- Map color/roughness/normal inputs consistently with color-space declarations.
- Keep texture scale tied to material coordinates and grain, not screen or drape dimensions.
- Provide opt-in visual fixtures; default CPU tests validate render data without GPU workloads.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricSurfaceModel.js


## EXO-LOOK-003

**Generate deterministic procedural textile maps**

Historical status: blocked. Technical dependencies: EXO-LOOK-001.

**Starting points**

- src/appearance/textiles.rs
- tests/textile_maps.rs
- src/lib.rs

**Acceptance ideas**

- Use declared seeds, dimensions and units for a small weave/knit subset.
- Check tiling, finite values, channel meaning and bounded memory.
- Label generated optics as visual appearance, not yarn-level simulation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/textileProcedurals.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricPbrGenerator.js


## EXO-LOOK-004

**Import and manage fabric texture assets**

Historical status: blocked. Technical dependencies: EXO-LOOK-001.

**Starting points**

- src/appearance/assets.rs
- src/atelier/fabrics_ui.rs
- tests/appearance_assets.rs
- src/atelier.rs
- src/lib.rs

**Acceptance ideas**

- Import/relink supported local maps into portable project assets with size limits.
- Distinguish sRGB albedo and linear data channels; preserve original file provenance.
- A missing map produces a visible fallback and never changes physical fabric parameters.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricAssetRegistry.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/fabricPbrFileNaming.js


## EXO-LOOK-005

**Define material-space garment graphic placements**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/garment/graphics.rs
- tests/graphic_placements.rs
- src/garment/mod.rs
- src/garment/template.rs
- src/garment/presets.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/authoring/clipboard.rs
- src/atelier/authoring_ui.rs
- src/fitting.rs
- docs/graphics.md

**Acceptance ideas**

- Persist bounded, uniquely identified material-space placements with stable panel IDs and opaque asset references; legacy documents load unchanged and existing valid panel IDs remain referencable. Unresolved asset IDs are metadata, not evidence of portable image loading.
- Reject invalid or dangling records atomically. Specify and test duplicate, remove, mirror, topology edits, clipboard and remesh behavior; preserve or explicitly reject references rather than silently lose or orphan them.
- Metadata-only edits and undo/redo preserve drape, physical mesh signature and worker/solver state. Exercise actual authoring/install paths and save/reopen, not only serialization or a reset flag.
- No third-party artwork fixtures. Asset storage/loading, rendering and native editing remain separate work; coordinate overlapping shared files with the open markings PR #6 before implementation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/nativeGarmentGraphics.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/178
- https://github.com/erythrofr/ClothCraft/pull/180
- https://github.com/erythrofr/ClothCraft/pull/182


## EXO-LOOK-006

**Render and edit garment graphics**

Historical status: blocked. Technical dependencies: EXO-LOOK-005.

**Starting points**

- src/render/graphics.rs
- src/atelier/graphics_ui.rs
- tests/graphics_ui.rs
- src/atelier.rs
- src/render.rs
- src/cloth.wgsl
- src/document.rs
- src/fitting.rs
- src/garment/graphics.rs
- src/garment/template.rs
- src/authoring.rs
- src/authoring/clipboard.rs
- src/atelier/documents_ui.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- src/atelier/authoring_smoke.rs
- tests/graphic_placements.rs
- tests/shader.rs
- docs/graphics.md
- Cargo.toml
- Cargo.lock

**Acceptance ideas**

- Implement a usable native workflow to import PNG, place/rotate/scale it on the selected 2D panel, pick/delete and undo/redo; render the placement on the correct front/back draped 3D panel with explicit clipping and layering. Do not advertise unsupported image formats or vector execution.
- Persist portable project-owned image assets and placement identities across Save, Save As, reopen, duplicate and clipboard operations. Handle missing files, overwrite/collision and partial write failures explicitly without corrupting the previous document or silently dropping artwork.
- Bound compressed and decoded PNG input, dimensions, asset counts and GPU resources; reject invalid data safely. Specify sRGB/linear and alpha conventions, preserve panel identity and existing diagnostic-overlay precedence, and do not change physical rest geometry, solver, drape or worker state for visual edits.
- Cover actual native control transactions with the named-widget CPU harness, asset serialization and portability, rejection/rollback, triangle UV preparation and WGSL validation. Record native-window/GPU visual acceptance separately, opt-in with current authorization; use only original or suitably licensed fixtures.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/nativeGarmentGraphics.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/nativeGraphicStorage.js


## EXO-LOOK-007

**Define reproducible stage, lighting and camera settings**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/atelier/stage_settings.rs
- tests/stage_settings.rs
- src/atelier.rs
- src/render/appearance.rs
- src/render.rs
- src/cloth.wgsl
- tests/shader.rs
- docs/stage-settings.md

**Acceptance ideas**

- Persist camera/environment/light settings separately from solver inputs.
- Preserve current background/floor controls and existing preferences.
- Validate values and asset references; decorative geometry is not a physical collider.
- Expose bounded directional-light rotation and ambient/key intensity controls through the existing Presentation UI. Wire both legacy/diagnostic and authored-optics shader paths to the same presentation settings; defaults preserve current lighting. Do not add fake HDRI or asset controls.
- Keep old preferences and fixed-rig presentation snapshots loadable with documented defaults. Save the new configurable rig with explicit version/identity validation; reject unsupported assets or invalid snapshots without partial application.
- Validate host/WGSL uniform layout and named controls on CPU; use separately authorized native captures to check directional changes and intensity endpoints. Presentation edits must preserve garment/history/worker state. Clearly distinguish illustrative lighting and floor-shadow presentation from calibrated materials or physical collision.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/createStageEnvironment.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/sceneCameraViewport.js
- https://github.com/erythrofr/ClothCraft/blob/54930c6487990e0bacec851495df01f3fecd601b/docs/stage-settings.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/121
- https://github.com/erythrofr/ClothCraft/pull/157


## EXO-LOOK-008

**Render stages and export still images**

Historical status: blocked. Technical dependencies: EXO-LOOK-007.

**Starting points**

- src/render/stages.rs
- src/atelier/stage_ui.rs
- tests/stage_render_data.rs
- src/atelier.rs
- src/render.rs

**Acceptance ideas**

- Provide original or redistributable stage fixtures with provenance.
- Render/export selected resolution with optional overlays and declared color space.
- Record camera/environment state for repeatability; no claimed path tracing or garment-fit certification.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/createStageEnvironment.js


## EXO-AV-001

**Define avatar landmarks and body measurements**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/avatar_measurements.rs
- tests/avatar_measurements.rs
- src/lib.rs

**Acceptance ideas**

- Separate visible mesh, collision mesh and named body landmarks.
- Store stable units and explicit user-confirmed measurement conventions.
- Test synthetic bodies and missing/ambiguous landmarks without guessing a size.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/seamlyMeasurements.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_UI_RENDERING_AND_AVATARS.md
- https://github.com/erythrofr/ClothCraft/pull/57

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/57


## EXO-AV-002

**Add body-aware panel arrangement assistance**

Historical status: blocked. Technical dependencies: EXO-AV-001.

**Starting points**

- src/garment/arrangement.rs
- src/atelier/arrangement_ui.rs
- tests/arrangement.rs
- src/garment/mod.rs
- src/atelier.rs

**Acceptance ideas**

- Offer placement previews from named landmarks with reversible commit.
- Preserve rest shape/grain and expose overlap/initial-separation diagnostics.
- Reject invalid initialization rather than silently teleporting simulated cloth.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/patternBodyArrangement.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/autoFitPlacement.js


## EXO-AV-003

**Expose pin editing and an explicit freeze policy**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/atelier/pins_ui.rs
- src/authoring.rs
- tests/pin_authoring.rs
- src/atelier.rs
- src/garment/pins.rs
- src/garment/mod.rs
- src/garment/template.rs
- src/garment/presets.rs
- src/atelier/authoring_ui.rs
- docs/pins.md

**Acceptance ideas**

- Reuse material-space pin_points instead of persisting mesh indices.
- Define pin versus freeze semantics and preserve undo/reload/remeshing behavior.
- Test release motion and stale-worker invalidation on CPU; GPU validation remains opt-in.
- Pin at the current drape and release without returning the panel to its flat arrangement. Persist material-space identity and world targets through ordinary template history and save/reload; preserve legacy documents and cache signatures when the new fields are absent.
- Whole-panel freeze retains collision participation. Reject unsupported topology changes atomically with an explicit release-first error; never silently lose holds or retarget arbitrary mesh indices.
- Test held vertices, release from zero held velocity, unchanged free-panel motion, invalid-target rollback, undo/redo, serialization, remesh policy and production worker-generation invalidation. No position projection or disabled contacts to conceal invalid initialization.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/backends/protocol.js


## EXO-AV-004

**Show avatar collision coverage and initialization problems**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/avatar/diagnostics.rs
- src/avatar.rs
- src/atelier/avatar_diagnostics.rs
- src/atelier/chrome.rs
- tests/avatar_diagnostics.rs
- docs/avatar-diagnostics.md

**Acceptance ideas**

- Report actual collider geometry identity, bounds and supported static-mesh assumptions.
- Show diagnostics separately from cosmetic avatar appearance.
- No collision approximations may be silently substituted to improve appearance/performance.
- Expose a read-only native inspection action with explicit freshness, coverage and unavailable checks; no per-frame mesh analysis or mutation of the solver/contact cache.
- Exercise closed, open, nonmanifold and inconsistently oriented fixtures plus named-widget refresh/staleness behavior on CPU; topology checks must not be described as proof of no intersections or valid initial separation.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/meshAvatarContact.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/96


## EXO-UX-001

**Publish a typed command capability registry**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/atelier/commands.rs
- tests/command_registry.rs
- src/atelier.rs

**Acceptance ideas**

- List only implemented commands with context, shortcuts, undo behavior and backend requirements.
- Unavailable commands show explicit reasons and cannot dispatch fake actions.
- Keep the registry usable by toolbar and future automation adapters.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/workspaceCommandIndex.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/clo7NativeToolCapabilities.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/29
- https://github.com/erythrofr/ClothCraft/pull/97


## EXO-UX-002

**Add a searchable command palette**

Historical status: ready. Technical dependencies: EXO-UX-001.

**Starting points**

- src/atelier/command_palette.rs
- tests/command_palette.rs
- src/atelier.rs

**Acceptance ideas**

- Search aliases and invoke the same validated commands as current UI.
- Respect text focus, Escape, keyboard navigation and disabled contexts.
- Keep radial menu intact; palette dispatch does not duplicate authoring logic.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/workspaceCommandIndex.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/110


## EXO-UX-003

**Persist workspace layouts and tool visibility**

Historical status: ready. Technical dependencies: EXO-UX-001.

**Starting points**

- src/atelier/workspace_profiles.rs
- tests/workspace_profiles.rs
- src/atelier.rs

**Acceptance ideas**

- Save/restore panel split, visible tool groups and view preferences with schema defaults.
- Invalid/offscreen dimensions recover to a usable layout.
- Workspace changes never modify garment document or solver settings implicitly.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/workspaceProfiles.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/studioCustomization.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/114


## EXO-UX-004

**Add group 3D transforms and marquee selection**

Historical status: ready. Technical dependencies: EXO-CORE-001.

**Starting points**

- src/atelier/selection.rs
- src/atelier/gizmo.rs
- src/atelier/authoring_ui.rs
- src/authoring.rs
- tests/group_placement.rs
- src/atelier.rs

**Acceptance ideas**

- Define visible-only versus through selection and the group transform pivot.
- Commit one transaction, preserving each panel rest shape and explicit drape reset policy.
- Test cancel/undo, linked pairs, hidden geometry and generation invalidation.
- For remaining headless acceptance, extract the existing CPU install/cancel transitions behind thin Atelier wrappers and test those production functions. Preserve reset/selection/history semantics; do not create a test-only replacement installer or change physics.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/meshSelection.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/111


## EXO-UX-005

**Create a provenance-aware garment template browser**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/template_catalog.rs
- src/atelier/template_browser.rs
- tests/template_catalog.rs
- src/lib.rs
- src/atelier/authoring_ui.rs
- src/atelier/documents_ui.rs
- docs/template-browser.md

**Acceptance ideas**

- Index bundled original templates with preview, metadata, units and source/license.
- Validate templates before opening; loading replaces a document only after user-visible confirmation.
- External patterns are explicit imports, not automatically downloaded or declared redistributable.
- Exercise the real browser widgets through the CPU named-widget harness, including cancel, confirmation and invalid input.
- Opening a template starts a new document: preserve the current document on cancellation or validation failure; clear its saved path and undo history only after successful replacement.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/openSourcePatternToolCatalog.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/projectFileLibrary.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/63


## EXO-UX-006

**Expose a versioned headless authoring session adapter**

Historical status: done. Technical dependencies: EXO-CORE-001, EXO-REV-005, EXO-UX-001.

**Starting points**

- src/bin/cloth-session.rs
- src/session_protocol.rs
- tests/session_protocol.rs
- src/lib.rs

**Acceptance ideas**

- Route supported requests through existing transactional commands with expected revision.
- Report capability/version, invalid input, stale revisions, cancellation and undo coherently.
- Keep transport local and do not execute arbitrary code or shell fields from requests.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RUST_CONVERSION_STATUS_2026-10-18.md
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/clothcraftApiClient.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/123
- https://github.com/erythrofr/ClothCraft/pull/125
- https://github.com/erythrofr/ClothCraft/pull/131


## EXO-QA-001

**Build public cross-editor behavioral fixtures**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- tests/fixtures/rewrite/
- docs/rewrite-fixtures.md

**Acceptance ideas**

- Create original small garments covering curves, holes, partial seams, trims and invalid inputs as supported.
- Pin each source model/revision and record losses when converting between representations.
- Do not redistribute private garments, upstream proprietary references or unreviewed EXO assets.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_VALIDATION_TEST_PLAN.md
- https://github.com/erythrofr/ClothCraft/pull/55

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/55


## EXO-QA-002

**Package a clean-machine Windows test release**

Historical status: ready. Technical dependencies: none.

**Starting points**

- scripts/package-windows.ps1
- .github/workflows/release-windows.yml
- docs/install.md

**Acceptance ideas**

- Package executable and required redistributable assets/notices with version/hash metadata.
- Verify launching/opening/saving from a clean directory with spaces in its path.
- Provide clear prototype/unsupported-hardware notes; do not require an AI subscription to install.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RELEASE_STATUS.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/82


## EXO-QA-003

**Verify native macOS and Linux build paths**

Historical status: done. Technical dependencies: none.

**Starting points**

- .github/workflows/build.yml
- docs/platform-validation.md

**Acceptance ideas**

- Build the CPU core and desktop targets on named platform/toolchain versions.
- Document platform-specific dialogs, paths, clipboard and GPU backend limitations.
- A successful compile is distinct from native interaction/GPU acceptance.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_RELEASE_STATUS.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/45


## EXO-PHY-001

**Define capabilities for real interchangeable solver adapters**

Historical status: done. Technical dependencies: EXO-CORE-001.

**Starting points**

- docs/solver-adapter-contract.md
- src/solver_lab/mod.rs

**Acceptance ideas**

- Specify lifecycle, rest/state ownership, collision/material/seam capabilities and error semantics.
- Map existing real adapters to declared capabilities without exposing failed experiments as editor modes.
- Retain current VBD/OGC production behavior and explicit physical-time reporting.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/backends/registry.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/backends/protocol.js

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/59


## EXO-PHY-002

**Compare candidate engines on shared quality gates**

Historical status: blocked. Technical dependencies: EXO-PHY-001, EXO-QA-001.

**Starting points**

- docs/solver-comparison-protocol.md
- examples/solver_compare.rs

**Acceptance ideas**

- Pin exact rest mesh/material/seams/x/v/time/control protocol and collision scope.
- Report pull/hold/release, separation, convergence and full-step cost before performance rankings.
- Do not interpret a PPF connector or visible preview as validated garment physics.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/docs/EXO_SIMULATION_RESEARCH.md


## EXO-PHY-003

**Evaluate foam and padding as a separate material experiment**

Historical status: blocked. Technical dependencies: EXO-PHY-001.

**Starting points**

- docs/padding-experiment.md
- examples/padding_probe.rs

**Acceptance ideas**

- Define dimensional material and contact assumptions distinct from thin cloth.
- Check compression/release energy and geometry against analytic or measured references.
- Do not add UI claims or replace cloth mechanics until a real model passes its gates.

**Evidence**

- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/foamCompressionModel.js
- https://github.com/mikristian/exo-sample-3d/blob/352c6086b622caeda95f16d9bd923624b95e05d9/src/core/foamPaddingGeometry.js


## EXO-CAD-009

**Integrate the freehand fitter and create new panels in the 2D workspace**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/atelier/board.rs
- src/atelier/authoring_ui.rs
- src/authoring.rs
- tests/freehand_authoring.rs
- docs/authoring.md
- docs/handoff.md

**Acceptance ideas**

- First verify and integrate PR #27 after successful CI; do not substitute or duplicate its fitter.
- Capture a stroke, preview fitted asymmetric cubics and the closing edge, then commit only a valid new panel.
- Cancel, undo/redo and save/reload preserve existing panels, seams and stable IDs.
- Reject invalid/self-intersecting contours and sample/segment overflow without mutating the document.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/27

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/27
- https://github.com/erythrofr/ClothCraft/pull/35


## EXO-SEW-006

**Expose cached sewing diagnostics in the native inspector**

Historical status: done. Technical dependencies: EXO-SEW-001.

**Starting points**

- src/atelier/sewing_ui.rs
- src/atelier/authoring_ui.rs
- docs/sewing-audit-ui.md

**Acceptance ideas**

- Expose a deliberate Audit/Refresh action in the sewing inspector using garment::seam::audit; never compile a report every rendered frame.
- Display seam IDs, material lengths and signed mismatch in millimetres, local easing and errors without claiming simulation convergence or measured seam gap.
- Cache results and explicitly mark them stale after the project generation changes; never display stale diagnostics as current.
- Allow selecting an existing seam for inspection/editing through the existing draft path without mutating the document or history until Apply.
- Cover cached-state freshness, malformed input reporting and headless egui presentation on CPU; keep GPU/native-window acceptance separate.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/41
- https://github.com/erythrofr/ClothCraft/blob/main/docs/sewing-audit.md

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/46


## EXO-QA-004

**Exercise editor tools with a deterministic named-widget harness**

Historical status: done. Technical dependencies: EXO-SEW-001.

**Starting points**

- src/atelier/authoring_smoke.rs
- src/atelier/authoring_smoke/widget_harness.rs
- src/atelier/sewing_ui.rs
- src/atelier/authoring_ui.rs
- docs/widget-harness.md

**Acceptance ideas**

- Register current-frame named widgets and reject missing, duplicate, disabled, stale or clipped targets explicitly.
- Inject pointer press/release, bounded drag, keyboard and text events through real egui controls with deterministic frame timing; never substitute direct model calls for UI interaction.
- Exercise sewing-audit refresh, inspection and stale state, plus numeric construction controls; assert resulting state and cancellation rather than only dispatched input.
- Run the new scenarios without a window, OS input injection or GPU initialization; preserve existing opt-in native smoke paths.
- Document scenario authoring, stable widget names, actionable failure output and limits of headless acceptance.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/46

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/49


## EXO-CAD-010

**Place a pattern point at the intersection of two reference edges**

Historical status: done. Technical dependencies: EXO-CAD-001, EXO-CAD-002, EXO-QA-004.

**Starting points**

- src/authoring.rs
- src/atelier/authoring_ui.rs
- tests/drawing_commands.rs
- docs/authoring.md

**Acceptance ideas**

- Resolve two straight reference edges by stable panel/boundary IDs in board space, including rotated panels, using the existing Rust intersection primitive.
- Expose Intersection in the point inspector with two edge selectors and a preview of both lines and the proposed point; reject missing, curved, degenerate and parallel references explicitly.
- Do not mutate before Apply; validate through the existing transaction and preserve exact undo/redo and other panels.
- Use the named-widget CPU harness for real selector/mode/Apply events and invalid/stale cases, plus rotated geometry and undo regressions.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/26
- https://github.com/erythrofr/ClothCraft/pull/49

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/52


## EXO-CAD-011

**Edit independent curve points and mirrored splits in the native 2D workspace**

Historical status: blocked. Technical dependencies: EXO-REV-001, EXO-REV-002, EXO-CAD-003.

**Starting points**

- src/garment/polygon.rs
- src/garment/curves.rs
- src/garment/domain.rs
- src/garment/presets.rs
- src/garment/shapes.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- tests/curve_point_authoring.rs
- docs/authoring.md
- docs/handoff.md

**Acceptance ideas**

- Expose Add/Edit/Remove curve point and Split at point on a selected named outer boundary in the native Piece inspector and 2D board, with millimetre input and visible handles/preview; do not substitute a freehand-new-panel tool.
- Keep stable authored point/boundary identities and cubic geometry through split, including knot-to-vertex promotion and verified linked mirrored splits as one atomic edit.
- Splitting any straight or cubic Bezier boundary, including Bezier-rounded corners, must preserve the original authored contour within floating-point tolerance using exact subdivision (de Casteljau for cubics). Preserve tangent continuity, propagate atomically to linked mirrored panels, and correctly migrate stable boundary identifiers, sewing references and persistent measurements, or reject unsupported reference migrations without mutation. A split must not flatten or resample the authored curve.
- Use the reviewed topology-reference mapping for partial/chain seams, measurements, target lengths and markings; reject ambiguous or unsupported edits with an actionable message and no mutation.
- Prove cancel, invalid input, text focus, one-step undo/redo, JSON reopen and rotated-layout editing using real named egui controls plus independent curve geometry checks.
- Preserve current linked-panel semantics, material rest metres and state invalidation; no EXO interface or session dependency.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/issues/13

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/66


## EXO-MFG-015

**Create and manage saved pattern measurements in the native 2D UI**

Historical status: ready. Technical dependencies: EXO-REV-001.

**Starting points**

- src/garment/polygon.rs
- src/garment/domain.rs
- src/garment/presets.rs
- src/garment/shapes.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/fitting.rs
- src/atelier/measure.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- tests/persistent_pattern_measurements.rs
- docs/authoring.md
- docs/handoff.md
- src/garment/markings.rs
- src/garment/svg_import.rs
- src/pattern_import.rs
- src/authoring/linked.rs
- tests/pattern_markings.rs
- tests/persistent_pom_references.rs
- src/atelier/documents_ui.rs

**Acceptance ideas**

- Complete the missing saved-model and transactional authoring integration together with the native UI, reviewing the schema/geometry foundation in PR #227 and the existing #66 proposal; do not require completion of unrelated cutline or solver work in broad review task #13.
- Extend the current temporary ruler with an explicit Save measurement workflow: choose two stations on one panel's named boundaries, give a label, and save a stable material-space measurement.
- Show selectable saved measurements and their straight-line rest-space distance in millimetres; support rename/delete and display overlays using the compiled material boundaries.
- Reuse the reviewed POM representation, with boundary arclength fractions and stable IDs; preserve or explicitly reject references through split, mirror, duplicate, delete and boolean operations.
- Keep metadata-only changes from remeshing or disturbing the drape; cover history, v1/v2 compatibility and save/reopen.
- Exercise create/rename/delete/cancel, invalid or stale references and keyboard focus through real named UI controls on CPU.
- Scope this PR to a useful single-size pattern ruler; exclude body/geodesic measurements, draped lengths, grading and tolerance tables.
- Keep the physical mesh fingerprint, metadata-only installer, playback and worker generation coherent; test the real shared installer on CPU in addition to prepared commands and persistence. Constructor-only additions to importers, presets and marking fixtures must remain mechanical.
- Protect new and edited measurement drafts when selecting another measurement, starting a new measurement or switching panels, with explicit save/discard/cancel decisions where needed. Include drafts in document Save/New/Open/Close handling; unchanged saved selection is not dirty, and Save must not silently omit a draft.
- Migrate saved and active unsaved stations through supported edits using operation-specific boundary identity and material-arclength mappings, with independent multi-landmark geometric validation informed by test-only PR #83. Cover the reported edge_2 t=0.75 / 0.15 m split drift through production edits and named Apply controls; preserve curve/vertex metadata and mirror semantics, and reject unsupported, degenerate or ambiguous mappings atomically. PR #66 remains an unmerged reference, not an imported implementation.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/issues/13

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/pull/227


## EXO-CAD-012

**Add editable pattern text annotations to the native Piece inspector and board**

Historical status: blocked. Technical dependencies: EXO-REV-001.

**Starting points**

- src/garment/polygon.rs
- src/garment/domain.rs
- src/garment/presets.rs
- src/garment/shapes.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/fitting.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- tests/pattern_annotations.rs
- docs/authoring.md
- docs/handoff.md

**Acceptance ideas**

- Provide Add label, text/position editing and Delete in the native Piece inspector; show actual saved labels on the 2D board using panel layout transforms.
- Use stable annotation IDs and local material coordinates from the reconciled markings model; do not introduce a parallel annotation representation.
- Support undo/redo and JSON reopen, with explicit mirror/duplicate/delete behavior and finite, unit-aware position validation.
- Keep labels as metadata: no mesh, mass, grain, sewing or drape changes when adding or editing text.
- Use CPU named-widget scenarios for create/edit/delete/cancel, rotated layouts, invalid values and text-field keyboard ownership.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/1
- https://github.com/erythrofr/ClothCraft/issues/12
- https://github.com/erythrofr/ClothCraft/pull/66

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/1
- https://github.com/erythrofr/ClothCraft/pull/66


## EXO-SEW-007

**Preview and create a cut-and-sewn dart from the native pattern UI**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/authoring.rs
- src/authoring/geometry.rs
- src/garment/polygon.rs
- src/garment/seam.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- tests/physical_dart_authoring.rs
- docs/authoring.md

**Acceptance ideas**

- Select a supported unsewn straight boundary and expose Dart with intake position/width and depth in millimetres; show cut legs and apex before explicit Apply.
- Apply one validated transaction that removes the triangular intake and creates the reversed dart-leg sewing relation in the existing implicit objective; preserve stable boundary references and mass/area accounting.
- Reject unsupported curved/sewn/annotated boundaries, incompatible references, invalid or intersecting geometry, and failed compilation without altering the document.
- Support Cancel, one-step undo/redo, save/reopen and coherent remeshing/cache invalidation; do not add a post-solver position projection.
- Exercise real named UI controls on CPU and verify analytic cut area, seam-leg correspondence and unaffected panels. Describe this as a cut-and-sewn dart, not uncut fold mechanics or proven fit.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/issues/13
- https://github.com/erythrofr/ClothCraft/pull/80

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/7
- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/pull/80
- https://github.com/erythrofr/ClothCraft/pull/85


## EXO-PHY-004

**Expose correctly timed contact and residual diagnostics in the native Simulation inspector**

Historical status: done. Technical dependencies: none.

**Starting points**

- src/vbd.rs
- src/vbd/audit.rs
- src/gpu_vbd.rs
- src/atelier/worker.rs
- src/atelier/chrome.rs
- tests/vbd_final_diagnostics.rs
- docs/solver-challenge.md
- docs/handoff.md

**Acceptance ideas**

- Review the final-state diagnostic changes in PR #66 against current upstream, including its existing offline audit, directional bounds and sparse precision refinement; submit a separate focused diagnostic PR.
- Provide an explicit native Simulation inspector action or opt-in sampling control showing accepted-state contact separation/count and free-DOF residual with units, sample time, freshness and backend availability.
- Distinguish accepted-state measurements from pre-update contact detection and mixed-color gradients, and unconstrained residual from constrained equilibrium/KKT; never display an unavailable GPU value as zero.
- Verify pinned/free normalization, inertia, seam and contact gradients with independent CPU oracles and state-preservation checks; include an actual named-widget control/presentation test.
- Measure complete step overhead on matched CPU scenes and keep expensive resampling opt-in unless the performance gate supports a default change; distinguish compute, step and rendering time.
- Preserve collision coverage, predictor, accepted trajectory and rejection policy. The proposed fixed 2 micrometre GPU rejection gate is a separate physics decision and is not authorized by this diagnostic UI task.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/13#issuecomment-6376032311
- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/pull/76
- https://github.com/erythrofr/ClothCraft/issues/72#issuecomment-6411088338

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/66
- https://github.com/erythrofr/ClothCraft/pull/76


## EXO-IO-009

**Integrate static avatar import into the workspace**

Historical status: done. Technical dependencies: EXO-IO-007, EXO-UX-005.

**Starting points**

- src/avatar_import.rs
- src/lib.rs
- src/atelier/avatar_import_ui.rs
- src/atelier/documents_ui.rs
- tests/avatar_import.rs
- docs/avatar-import.md

**Acceptance ideas**

- Expose explicit local OBJ/glTF/GLB import with source, counts, bounds, metre units and loss warnings before confirmation; reject ambiguous scene selection without silent fallback.
- Prepare validated canonical OBJ geometry with an owned staging lifetime; preserve v1 project compatibility and portable save/recovery. Never overwrite the source asset.
- Explain and apply drape reset on avatar replacement, preserving authored rest geometry, seams and live simulation settings. Cancel and preparation failure leave the active project and worker generation unchanged.
- Invalidate old worker responses on successful commit, clear transient interactions and mark the document dirty. Saved assets survive staging cleanup.
- Test actual named controls and shared production transactions on CPU: cancel, invalid input, transforms, save/reload after staging cleanup and old OBJ compatibility.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/62
- https://github.com/erythrofr/ClothCraft/issues/64

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/74


## EXO-IO-010

**Import reviewed DXF contours as editable native panels**

Historical status: done. Technical dependencies: EXO-IO-002.

**Starting points**

- src/pattern_import.rs
- src/lib.rs
- src/atelier/pattern_import_ui.rs
- src/atelier/documents_ui.rs
- tests/pattern_import.rs
- docs/pattern-import.md

**Acceptance ideas**

- Expose local DXF selection, explicit source scale/units and a bounded 2D preview with layer, entity, closure and unsupported-entity information before confirmation. Missing units require user input; never silently guess.
- Let the user explicitly select closed contours to create unsewn editable panels. Keep cut/sew source paths separate; reject ambiguous nested, overlapping or self-intersecting contours rather than guessing holes, seams or manufacturing semantics. Open paths remain visible as excluded geometry.
- Prepare and validate the complete addition before changing the active garment. Allocate stable panel/boundary IDs, preserve existing rest geometry, seams, fabrics and avatar, and append the selected panels as one undoable edit. Explain the dependent drape reset before Apply.
- Cancel, invalid files and compilation failures preserve document/history/worker generation. Successful Apply marks dirty, resets dependent caches and discards stale worker responses. Save/reopen and undo/redo preserve imported material dimensions.
- Use named UI controls and production transactions on CPU to exercise scale confirmation, contour selection, losses, Cancel, one Apply, failure rollback, existing-panel preservation and save/reload. Include independent DXF fixtures and document generic-DXF limitations; do not claim industrial dialect support.

**Evidence**

- https://github.com/erythrofr/ClothCraft/pull/88
- https://github.com/erythrofr/ClothCraft/issues/87
- https://github.com/erythrofr/ClothCraft/issues/90#issuecomment-6413493886

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/92


## EXO-IO-011

**Import reviewed SVG patterns in the native workspace**

Historical status: done. Technical dependencies: EXO-IO-001.

**Starting points**

- src/atelier/svg_import_ui.rs
- src/atelier/documents_ui.rs
- src/atelier/authoring_smoke.rs
- docs/svg-import.md

**Acceptance ideas**

- Expose File > Import SVG patterns with bounded local UTF-8 reading, source identity, physical dimensions, explicit curve tolerance, loss report and outline preview before applying. No external resource resolution or guessed scale.
- Preview SVG +Y downward and let the user select outlines, an existing fabric and mesh spacing. Changing source or tolerance invalidates reviewed selection and acknowledgement; stale previews cannot apply.
- Reuse garment::svg_import preparation and the existing authoring/history installer. Add stable unsewn panels in a separated arrangement; preserve existing rest geometry, materials, seams, avatar and unchanged-panel drape; invalidate stale worker responses and mark dirty.
- Cancel, Escape, invalid input and failed compilation preserve document/history/generation. Respect current authoring drafts and the normal unsaved-window-close flow.
- Use named-control CPU tests and real shared transactions for selection/acknowledgement, stale Apply, cancellation, one apply, rollback, drape preservation, undo/redo and portable save/reopen. Document native picker/GPU acceptance separately.
- Extend the existing opt-in native authoring smoke with a public SVG fixture and actual dialog responses through svg_import_tick/install_edit. Verify one history entry, dirty state, unchanged-panel state preservation and invalidation of an outstanding worker result; a dispatched click or prepared Edit alone is not acceptance. Use bounded failure-reporting phases. A smoke-only file adapter may choose an isolated output-local fixture, but normal user file dialogs and other smoke scenarios must remain unchanged. Keep GPU execution opt-in and out of default CPU CI.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/103
- https://github.com/erythrofr/ClothCraft/pull/101

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/107
- https://github.com/erythrofr/ClothCraft/pull/149
- https://github.com/erythrofr/ClothCraft/pull/155


## EXO-QA-005

**Trace native appearance history through actual UI controls**

Historical status: done. Technical dependencies: EXO-QA-004.

**Starting points**

- src/atelier/authoring_smoke.rs
- src/atelier/fabrics_ui.rs
- src/atelier/documents_ui.rs
- docs/native-appearance-acceptance.md

**Acceptance ideas**

- Reuse the opt-in native authoring smoke entrypoint and actual control responses for appearance Apply, pointer Undo/Redo and keyboard history. Missing candidate support or controls must fail explicitly, never skip to success.
- Emit structured observations of authored optics, history availability, playing state, worker generation and x/v/time. Check paused state equality and preservation of an in-flight worker response; do not bypass the normal edit/history installer.
- Exercise text keyboard ownership, restoration of default optics and isolated public-fixture save/reopen. The native smoke document adapter may select only fresh output-local fixture paths, reuse normal Save/Open installation, and bypass interactive recovery/file dialogs only for this explicit scenario. Preserve ordinary user document behavior and all other smoke modes. Distinguish a dispatched event from a verified state change and fail on timeout, no-op or rejected history.
- Keep GPU execution opt-in and outside default CPU tests/CI. Document exact candidate revision, commands, partial failures and unverified visual appearance. Production fixes outside this scope require a separate coordinated handoff.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/135
- https://github.com/erythrofr/ClothCraft/pull/128#issuecomment-6424222657

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/128
- https://github.com/erythrofr/ClothCraft/pull/137


## EXO-UX-009

**Expose native authoring tools in the command palette**

Historical status: ready. Technical dependencies: EXO-UX-001.

**Starting points**

- src/atelier/commands.rs
- src/atelier/command_palette.rs
- src/atelier/board.rs
- src/atelier.rs
- tests/command_registry.rs
- docs/command-registry.md

**Acceptance ideas**

- Add descriptors and search aliases for existing native authoring tools; do not advertise nonexistent operations or unwired shortcuts.
- Dispatch through the same set_authoring_tool transition as the toolbar/radial menu, including existing move/rotate modes; preserve draft, focus, modal and runtime availability behavior.
- Expose the existing freehand mode through the palette using one shared activation transition with the board toolbar. Preserve cancellation, replace mode and draft semantics; do not advertise an unwired shortcut or alter the drawing algorithm.
- CPU tests cover stable unique registry keys, each tool mapping, palette search, keyboard activation, disabled contexts and existing input isolation. Native-window acceptance is separate and GPU opt-in.
- Keep Firefly-owned authoring_ui.rs untouched; no changes to garment physics, geometry, contact or solver.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/171

**Related contributions**

- https://github.com/erythrofr/ClothCraft/pull/173


## EXO-CAD-014

**Round pattern corners by radius and maintain Bezier continuity**

Historical status: blocked. Technical dependencies: EXO-CAD-003, EXO-CAD-011, EXO-MFG-015.

**Starting points**

- src/garment/curves.rs
- src/garment/polygon.rs
- src/garment/domain.rs
- src/garment/presets.rs
- src/garment/shapes.rs
- src/authoring.rs
- src/authoring/geometry.rs
- src/authoring/linked.rs
- src/atelier/authoring_ui.rs
- src/atelier/board.rs
- tests/corner_rounding.rs
- tests/linked_mirror.rs
- tests/persistent_pattern_measurements.rs
- docs/authoring.md
- docs/panel-tools.md
- docs/handoff.md

**Acceptance ideas**

- Fillet selected outer-contour corners in the native Piece inspector and 2D board using a positive numeric radius in millimetres, converted once to material metres. Preview the tangent points and resulting contour before Apply. Include a 90-degree straight-edge fixture with a 10 mm radius; handle supported curved-edge configurations explicitly and reject unsupported, ambiguous, degenerate, oversized or self-intersecting constructions without mutation.
- Construct the intended tangent circular fillet from the requested radius. If stored as ordinary cubic Beziers, document and independently verify a bounded geometric error in millimetres against that circle; never describe the cubic approximation as an exact circular arc. Separate authored fillet error from display/meshing flattening tolerance. No polygonal replacement of the authored fillet.
- Convert selected corners between sharp, smooth and symmetric Bezier modes. Sharp permits independent incident handles; smooth keeps opposite collinear tangents with independent handle lengths; symmetric keeps opposite collinear tangents with equal lengths. Define behavior for absent or zero-length handles and persist the chosen mode compatibly with existing documents.
- Automatically maintain the selected smooth/symmetric tangent rule when either handle moves, through the normal CPU authoring transaction and all existing handle-edit callers. Smooth means geometric tangent continuity (G1), not an unproved C1/C2 parameterization claim. Respect local material coordinates under rotated board layouts.
- Propagate fillets, mode conversions and handle coupling atomically to linked mirrored panels in either editing direction, using the existing reflection/link semantics exactly once. Preserve independent panel identities, layout, placement, fabric and grain policy; reject a failed partner operation without changing either piece.
- Preserve named boundary references and their defined material stations through supported edits, including whole/partial/chain seams, persistent measurements, target lengths and allowance/notch metadata. Define migrations for trimmed/removed intervals and new fillet boundary IDs; unchanged references must not drift silently. Reject unsupported or ambiguous references before mutation, with an actionable message. Reuse the reviewed topology-reference model and review pending measurement PRs #227/#228 rather than introducing a parallel representation.
- Keep geometry-preserving splitting in EXO-CAD-011 (#68). Splitting any resulting cubic fillet at a valid parameter uses exact de Casteljau subdivision within floating-point tolerance, preserves the original contour and tangent continuity, and never flattens or resamples the authored curve. Propagate linked mirrors and migrate references or reject atomically. This task creates/edits corners; it does not absorb #68's splitting implementation.
- Prepare and compile the complete edit before installation as one history entry. Preview/cancel, Escape, stale selection, invalid input, failed reference mapping or failed compilation leave the document, history and live state unchanged. Successful fillets/mode/handle edits support one-step undo/redo and ordinary JSON Save/Open, including persisted corner mode and subsequent coupled handle edits. Rebuild dependent caches after actual rest-geometry changes and preserve compatible unaffected-panel state.
- Use real named egui controls and independent CPU geometry/reference checks for radius and circle-error bounds, tangent directions, unequal smooth/equal symmetric handles, sharp independence, both mirror directions, seam/measurement preservation or refusal, rounded-curve subdivision, rotated layouts, text focus, cancel, undo/redo and reopen. Keep local checks at two CPU jobs/test threads; native-window/GPU acceptance remains separate and opt-in.

**Evidence**

- https://github.com/erythrofr/ClothCraft/issues/233
- https://github.com/erythrofr/ClothCraft/issues/68
- https://github.com/erythrofr/ClothCraft/issues/69
