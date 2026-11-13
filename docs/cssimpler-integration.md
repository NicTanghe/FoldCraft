# cssimpler integration

Both native desktop editors use cssimpler revision `34472cb`, which includes
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
