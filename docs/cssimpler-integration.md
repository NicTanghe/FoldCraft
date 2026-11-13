# cssimpler integration

Both native desktop editors use cssimpler revision `155ae1e`, which includes
captured event handlers and the transition update described in cssimpler's
`docs/reports/`. Each editor instance owns its action bitmask. Click handlers
capture a shared reference to that instance's mask; the update loop consumes its
own pending actions. Multiple instances therefore do not consume each other's
clicks. Repeated identical clicks before an update remain coalesced, as before.
An action invalidates the UI structure even while simulation is running.

The day/night switch uses CSS transitions. The pinned cssimpler can retain an
active transition through unrelated scene changes when it can match the affected
branches safely. Matching remains conservative for changed sibling lists without
unique IDs, and equal-length anonymous siblings are still matched by position;
this is not a complete browser transition model. Avoid relying on positional
identity when inserting or reordering animated siblings. Its report's synthetic
measurements are not FoldCraft frame timings. Captured handlers allocate shared
handler storage when the tree is built; no application performance gain is claimed.

The simulation worker clock remains independent of rendering. CPU-only and
headless desktop checks do not demonstrate native GPU viewport behavior; that
requires a separately authorized window/GPU run.

During an active layout transition, a paint-only refresh must use the transition's
destination layout as its template. Earlier cssimpler revisions reused the
intermediate sampled layout, so repeated simulation status updates could keep
the theme switch from reaching either endpoint. The upstream fix covers `App`
and `FragmentApp`, including partial refreshes; FoldCraft retains paint-only
worker updates and does not add an animation timer.

A subsequent upstream optimization reuses an existing transition plan when a
refresh changes only content and preserves target geometry, style and identity.
See cssimpler PR #10 and its CPU benchmark; geometry or style changes still use
the conservative rebase path. Native frame cadence has not been measured here.
