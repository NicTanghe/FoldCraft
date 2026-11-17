# Working on top of upstream

This repository is a fork of NicTanghe/FoldCraft (itself a fork of
erythrofr/ClothCraft). The fork adds an agent surface and a browser studio; it
does not change the numerics.

```sh
git remote add upstream https://github.com/NicTanghe/FoldCraft.git   # once
git fetch upstream
git merge upstream/main        # or rebase; upstream moves fast
```

## What is ours

| Layer | Files | Merge risk |
|---|---|---|
| MCP door | `src/mcp.rs`, `src/bin/cloth-mcp.rs`, `tests/mcp.rs` | additive |
| HTTP door | `src/api.rs`, `src/bin/cloth-server.rs` | additive |
| Shared session path | `src/session_client.rs`, `src/inspect.rs` | additive |
| Skills | `skills/`, `src/skills.rs` | additive |
| Studio | `studio/` | isolated |
| Registration | one line each in `src/lib.rs`, `Cargo.toml` | small, conflict-prone; resolve by hand |

## Rules

- **Never patch their numerics.** If a solver or meshing change is needed, send it
  upstream first and merge it back; a local numerics edit is a fork we can no
  longer sync.
- **Keep our layer additive.** New files and one-line registrations survive a
  merge; edits inside `authoring.rs` or the solver do not.
- **One exception, kept minimal:** `template_catalog::Selection::template()`
  exposes the validated template for the API. It is three lines and worth
  proposing upstream.
- **After a merge:** `cargo test --locked --no-default-features` and
  `npm run build` in `studio/`, then re-run the MCP tests. Upstream's own
  `scripts/check-cpu.ps1` is the fuller gate on Windows.
