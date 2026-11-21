# Stab Documentation

This directory contains Stab's active plans, architecture records, generated parity view, migration notes, and release procedure. `AGENTS.md` and `CLAUDE.md` are symlinks to this file.

## Active Sources

- [plans/GOAL.md](plans/GOAL.md) is the short current execution contract.
- [plans/stim-core-parity-and-lean-evidence-plan.md](plans/stim-core-parity-and-lean-evidence-plan.md) owns the scope, the active milestone, the test contract, and the acceptance criteria.
- [plans/open-items.md](plans/open-items.md) lists known gaps that no milestone owns.
- [plans/review-fix-plan.md](plans/review-fix-plan.md) records the plan and outcome for the findings of the 2026-11-19 code review, including Phase D, and lists the decisions and redesigns still open after it.
- [plans/open-after-phase-d.md](plans/open-after-phase-d.md) expands each item left open after Phase D with its current state, the proposed design, its cost, and a recommendation.
- [plans/stim-main-rebaseline-plan.md](plans/stim-main-rebaseline-plan.md) is the decided plan for moving the Stim baseline from v1.16.0 to current `main`; the work is not started.
- [stim-parity.md](stim-parity.md) is the generated Stim v1.16.0 feature and evidence view. Its source is `oracle/stim-v1.16-parity.toml` and the bounded family fragments it names.
- [architecture/](architecture/) owns durable product dependency rules and architecture decisions.
- [MIGRATING-0.2.md](MIGRATING-0.2.md) covers the component-crate and public-path migration.
- [RELEASING.md](RELEASING.md) covers coordinated crate and binary releases.
- [../benchmarks/SUITE.md](../benchmarks/SUITE.md) is the generated active performance matrix.
- [../benchmarks/current-aarch64-evidence.toml](../benchmarks/current-aarch64-evidence.toml) names the current controlled-host full and soak evidence.

Completed and superseded plans, progress reports, qualification dashboards, and feature inventories are retained in Git history instead of remaining beside the active contracts. The fearless SIMD plan, the engine speed and architecture plan, the Rust 1.96 to 1.100 adoption note, and the completed milestones of the parity plan are last present at commit `7b0aa4f3`; their durable decisions live in the architecture records and the migration guide.

## Correctness

Use `just oracle::parity-check` to validate the atomic parity ledger and every canonical owner selector. Use `just oracle::parity-run` to execute completed owners, and `just oracle::parity-render --check` to reject generated-view drift.

Use `just oracle::result-formats --check` for the checked byte-exact result-format corpus and live pinned-Stim differential. Use `just oracle::gates` for the canonical gate catalog comparison. Use `just oracle::run --all-rows` to compare every fixture manifest row against live pinned Stim or its stored pinned-Stim output; these supporting witnesses do not own semantic families. `two-sample` rows run the same seeded argv through pinned Stim and Stab and require every `01` output column, plus each declared `joint any=` or `joint parity=` statistic, to agree under a pooled two-proportion z-test whose Bonferroni false-positive bound is checked against the row's `false_positive_rate`; `--statistical` selects them together with the one-sample analytic rows. A two-sample column or statistic in which exactly one side is constant is tested with Fisher's exact conditional test instead of the z-test, a one-sample budget counts both Stim's and Stab's outputs, and Stim's exit status and stderr class are checked on every command row as Stab's are. `just oracle::record --check-clean` rechecks every stored exact-output golden against pinned Stim, with the core parse-print rows going through the `stab_stim_parse_print` libstim helper. Live DEM text parity requires a 128-bit `long double` host such as the AArch64 baseline.

Every surviving test must protect semantic, statistical, safety, resource, or user-visible behavior. Do not create tests whose only claim is a derive, label, re-export, constant, private pointer identity, or bookkeeping shape.

## Performance

[../benchmarks/suite.toml](../benchmarks/suite.toml) is the sole active performance source. `just bench::e2e-check` validates it without timing; formal and diagnostic workflows are documented in [../benchmarks/README.md](../benchmarks/README.md).

When changing public behavior, APIs, CLI flags, formats, operational workflows, or acceptance gates, update the matching active source and regenerate derived documentation in the same change.
