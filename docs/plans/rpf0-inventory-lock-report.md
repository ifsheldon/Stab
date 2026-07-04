# RPF0 Inventory Lock Report

## Summary

RPF0 locked the source-owned map from the partial feature checklist to the remaining partial feature milestones in `docs/plans/remaining-partial-feature-milestones.md`.
The current execution plan is `docs/plans/non-deferred-partial-feature-milestones.md`, which uses that RPF inventory as source material.
This report records the current inventory, oracle, benchmark, audit, and review evidence for that lock.

Status: complete against the RPF0 inventory-lock contract.

## Scope

Included:

- Classify every `Partial` checklist row as active, rollup, mixed, or deferred-only.
- Keep historical `pf1-` through `pf7-` oracle and benchmark ids as extraction contracts while mapping them to active RPF milestones.
- Add benchmark placeholders for every performance-sensitive row promised by the RPF plan.
- Keep deferred surfaces such as Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM, Quirk, Crumble, GPU, ecosystem packages, exact random-stream parity, C++ header compatibility, public simulator products, and deprecated `--detector_hypergraph` outside RPF implementation scope.

Excluded:

- Implementing any RPF1 through RPF7 feature behavior.
- Promoting report-only or contract-only benchmark placeholders into the primary threshold gate.
- Replacing manifest-only oracle rows with executable tests before their owning implementation milestones.

## Inventory Changes

- `docs/plans/non-deferred-partial-feature-milestones.md` is the active execution plan for the remaining non-deferred partial rows.
- `docs/plans/GOAL.md` points agents at the PFM plan and defines milestone work loops, benchmark rules, and stop conditions.
- `docs/plans/partial-feature-inventory.md` now maps the historical PF rows to active RPF milestones, locks owned/semantic-mining/deferred subcases for RPF1 through RPF7, and explicitly includes the `analyze_errors --decompose_errors` checklist row under both core analyzer and CLI analyzer owners.
- `benchmarks/manifest.csv` now has non-primary `contract-only` placeholders for every RPF2 through RPF7 benchmark row promised by the active plan.
- `oracle/fixtures/manifest.csv` keeps historical manifest-only `pf1-` through `pf7-` extraction rows, but their descriptions now point at locked RPF subcases instead of saying subcase splitting is future work.
- `docs/plans/partial-feature-closure-plan.md` is marked as historical planning context.
- `docs/stab-feature-checklist.md` and `docs/plans/rust-stim-drop-in-rewrite.md` point at the RPF plan as the active source.

## Completion Matrix

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Every partial row has an active owner, rollup owner, mixed owner, or deferral reason. | Satisfied | `docs/plans/non-deferred-partial-feature-milestones.md`; `docs/plans/partial-feature-inventory.md` | The plan contains the full partial-row coverage matrix, and the inventory maps implementation rows to historical PF ids plus RPF owners. |
| Exact owned subcases are locked before implementation starts. | Satisfied | `docs/plans/partial-feature-inventory.md` | The locked subcase section splits RPF1 through RPF7 into owned, semantic-mining, and deferred or out-of-scope subcases. |
| Oracle manifest has manifest-only rows for active implementation work items. | Satisfied | `oracle/fixtures/manifest.csv`; `just oracle::list` | Existing `pf1-` through `pf7-` manifest-only rows remain source-owned extraction contracts and point at locked RPF subcases. |
| Benchmark manifest has non-primary placeholders for milestones needing performance evidence. | Satisfied | `benchmarks/manifest.csv`; `just bench::list` | Added missing RPF2 through RPF7 placeholder rows as `non-primary-report-only` and `contract-only`, and the inventory names the matching rows. |
| Oracle and benchmark tooling parse the planned rows without adding them to the primary gate. | Satisfied | Verification commands below | `just oracle::list`, `just oracle::matrix --check`, `cargo test -p stab-oracle fixtures --quiet`, `cargo test -p stab-bench --quiet`, and `just bench::list` pass. |
| Historical PF planning context is not mistaken for the active execution contract. | Satisfied | `docs/plans/partial-feature-closure-plan.md`; `docs/plans/rust-stim-drop-in-rewrite.md`; `docs/stab-feature-checklist.md` | The older PF plan is explicitly historical, and the roadmap/checklist point at the RPF plan. |
| Milestone-audit and full-code-review are closed before claiming completion. | Satisfied | Sidecar reviewers and local review | Sidecar findings were fixed or classified as historical-report context. |

## Verification

Commands run:

```sh
git diff --check
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench --quiet
just oracle::list
just oracle::matrix --check
just bench::list
find . -path './target' -prune -o -path './.git' -prune -o -type f \( -name '*.rs' -o -name '*.md' -o -name '*.toml' \) -print0 | xargs -0 wc -l | sort -nr | head -40
just maintenance::pre-commit
```

Large-file review notes:

- No touched Rust source file is over the 1200-line threshold.
- Existing watch-list files remain above 900 lines, including `crates/stab-core/src/dem/tests.rs`, `crates/stab-core/tests/stim_format.rs`, `crates/stab-core/src/circuit.rs`, `ops/bench/src/baseline.rs`, and other previously large implementation files.
- This RPF0 slice adds docs and benchmark metadata only, so the watch-list files are residual risk rather than findings for this milestone.

## Audit And Review

Local milestone-audit result: complete after the benchmark placeholder, inventory mapping, locked-subcase, oracle-description, stale-checklist, and historical-report fixes.

Local full-code-review result: no code, CLI, file-format, security, SIMD, or public API changes were made in this slice.
The main review risk was documentation and benchmark metadata drift, addressed by syncing the active plan references and adding placeholder benchmark rows.

Sidecar milestone-audit:

- Finding: RPF0 still deferred exact owned subcase splitting to later milestones.
  Resolution: added locked RPF subcases to `docs/plans/partial-feature-inventory.md` and updated manifest-only oracle descriptions to reference those locked subcases.
- Finding: benchmark placeholders existed but the inventory benchmark-plan column omitted several manifest rows.
  Resolution: updated the inventory benchmark-plan cells to name all added RPF2 through RPF7 placeholders.
- Finding: stale PF/RPF wording remained in the inventory.
  Resolution: updated active-owner wording to use RPF terminology while preserving historical `pf*` row ids.

Sidecar full-code-review:

- Finding: feature checklist cited stale 72-comparable-row beta evidence.
  Resolution: updated `docs/stab-feature-checklist.md` to cite the current 80 comparable rows and 5 checked no-ratio waivers across 85 primary rows.
- Finding: `docs/plans/post-beta-fix-report.md` described sweep-conditioned conversion and `m2d --ran_without_feedback` as deferred even though later M9 work added scoped support.
  Resolution: marked those report statements as historical and pointed current readers to the checklist, M9 reports, and RPF plan.
- Residual risk: historical 72-row evidence remains in the post-beta report's evidence section because it records the state of that report, not the current beta gate.

## Remaining Follow-Ups

- Replace manifest-only oracle rows with executable rows during the owning RPF implementation milestones.
- Replace benchmark placeholders with real runners, measurement work units, compare notes, profiler notes, and optional thresholds only when the owning implementation milestone adds behavior.
- Keep report-only and contract-only rows out of the primary threshold gate until a later milestone records faithful comparison evidence.
