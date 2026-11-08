# Contributing to ClothCraft

Humans and AI agents are welcome. Start with [AI_START_HERE.md](AI_START_HERE.md), [AGENTS.md](AGENTS.md), the [handoff](docs/handoff.md) and the [roadmap](ROADMAP.md). Both human and AI contributions follow the same physical contracts and verification requirements.

## Choose a small, verifiable change

The project is a very early alpha. Pick one bounded task with observable acceptance cases. Save/open and partial-edge sewing already have implementations. Choose a remaining acceptance gap or one of the current `UX`, `PHYS`, `PERF` or `IO` briefs in the roadmap; do not reimplement a completed feature based on an old note. The [manual study](docs/manual-study.md) distinguishes public behavioral references, independently implemented features and work that remains planned. Do not describe roadmap formats or solver backends as available.

For physics changes, describe the affected energy/contact model, units, assumptions and limits. Add a test that could falsify the implementation: derivatives, objectivity, balance, rest-state preservation, an analytical result or convergence. Use the same scenes and settings when comparing solvers; report quality and full-step cost using the [solver challenge](docs/solver-challenge.md). Documentation and reversible visual changes do not need tests that merely repeat the implementation.

## You can help without writing code

Open an issue with the garment or a minimal example you are allowed to share, the steps you took, what you expected and what happened. Include mesh spacing, solver settings, app revision and whether the problem survives a restart. A short recording can clarify confusing interactions. Avoid sharing private assets accidentally.

Workflow sketches, measurement references and solver ideas are welcome. Label an idea as a hypothesis and state what observation would support or disprove it. Do not invent timings, reproduce a paper only in name or describe a proposed format as implemented.

The [contributor kit](docs/contributor-kit.md) includes prompts for an AI coding assistant. You remain responsible for reviewing its changes and reporting the checks actually run. A useful contribution can be one clear issue, one repeatable test or one small pull request.

## Validate without a GPU

```powershell
.\scripts\check-cpu.ps1
```

The CPU workflow in [.github/workflows/cpu.yml](.github/workflows/cpu.yml) runs these checks and a small headless assembly. Inspect its actual result in GitHub Actions. For desktop type changes, `cargo build --locked -j 2` compiles without launching the interface. Keep local checks to two CPU jobs/test threads.

GPU tests and window checks are opt-in. Coordinate their use with the machine's owner; never add GPU work to the default test suite.

## Prepare the contribution

- Preserve `Cargo.lock` and existing document compatibility. Keep examples small, deterministic and explicit about uncalibrated material values.
- Preserve stable panel/boundary IDs and material-space rest geometry. Rebuild dependent caches coherently after physical edits; never infer rest shape from a drape.
- Keep `target/`, `output/`, heavy data and simulation caches out of Git. Small evidence belongs in `docs/`, with reproduction commands and limitations.
- Include the concrete before/after behavior, relevant validation and any remaining failures in the PR. Update the handoff and module documentation when behavior changes.
- Reference sources and distinguish reading a paper/manual, independent implementation and reused code. Do not include proprietary code, icons, example garments or other unauthorized assets.

## License and provenance

ClothCraft's original source and documentation are under the [MIT License](LICENSE), chosen by the project owner. Contributions intended for inclusion must be compatible with that license; retain any applicable third-party notices. The bundled MakeHuman assets retain their [CC0 notice](assets/makehuman/LICENSE.ASSETS.md), and dependencies retain their own licenses. The [artwork record](assets/branding/README.md) documents the generated icon and its references.

## Git workflow

Follow [the software factory](docs/software-factory.md). Select tickets from
`implementation/queue.json`, communicate in gitignored `comms/`, and deliver
every feature through a PR merged with `gh`. Codex ticket/PR reviews are always
allowed; fix valid findings and explain declined suggestions. Commit, push, PR
creation and merge are authorized for assigned feature work. The former upstream
coordinator is not used.

If you reuse another contribution, preserve its authorship, license and source
references.
The first selective integration from FireflyTheReal / julienfoss is recorded in
[the integration notes](docs/firefly-integration.md).
