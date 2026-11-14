# Stab Documentation

This directory contains Stab's active plans, architecture records, generated parity view, migration notes, and release procedure. `AGENTS.md` and `CLAUDE.md` are symlinks to this file.

## Active Sources

- [plans/GOAL.md](plans/GOAL.md) is the short current execution contract.
- [plans/stim-core-parity-and-lean-evidence-plan.md](plans/stim-core-parity-and-lean-evidence-plan.md) owns the active rationale, milestones, tests, benchmarks, and acceptance criteria.
- [plans/fearless-simd-stable-rust-plan.md](plans/fearless-simd-stable-rust-plan.md) records the completed move from Nightly portable SIMD to `fearless_simd` on Stable Rust 1.100; it changed no parity scope.
- [plans/engine-speed-and-architecture-plan.md](plans/engine-speed-and-architecture-plan.md) records the completed user-scale speed, admission-limit, and execution-architecture work from the November review, with one checkpoint per milestone and the open items it leaves.
- [stim-parity.md](stim-parity.md) is the generated Stim v1.16.0 feature and evidence view. Its source is `oracle/stim-v1.16-parity.toml` and the bounded family fragments it names.
- [architecture/](architecture/) owns durable product dependency rules and architecture decisions.
- [MIGRATING-0.2.md](MIGRATING-0.2.md) covers the component-crate and public-path migration.
- [RELEASING.md](RELEASING.md) covers coordinated crate and binary releases.
- [../benchmarks/SUITE.md](../benchmarks/SUITE.md) is the generated active performance matrix.
- [../benchmarks/current-aarch64-evidence.toml](../benchmarks/current-aarch64-evidence.toml) names the current controlled-host full and soak evidence.

Superseded plans, progress reports, qualification dashboards, and feature inventories are retained in Git history instead of remaining beside the active contracts.

## Correctness

Use `just oracle::parity-check` to validate the atomic parity ledger and every canonical owner selector. Use `just oracle::parity-run` to execute completed owners, and `just oracle::parity-render --check` to reject generated-view drift.

Use `just oracle::result-formats --check` for the checked byte-exact result-format corpus and live pinned-Stim differential. Use `just oracle::gates` for the canonical gate catalog comparison. Use `just oracle::run --all-rows` to compare every fixture manifest row against live pinned Stim or its stored pinned-Stim output; these supporting witnesses do not own semantic families. `two-sample` rows run the same seeded argv through pinned Stim and Stab and require every `01` output column, plus each declared `joint any=` or `joint parity=` statistic, to agree under a pooled two-proportion z-test whose Bonferroni false-positive bound is checked against the row's `false_positive_rate`; `--statistical` selects them together with the one-sample analytic rows. Live DEM text parity requires a 128-bit `long double` host such as the AArch64 baseline.

Every surviving test must protect semantic, statistical, safety, resource, or user-visible behavior. Do not create tests whose only claim is a derive, label, re-export, constant, private pointer identity, or bookkeeping shape.

## Performance

[../benchmarks/suite.toml](../benchmarks/suite.toml) is the sole active performance source. `just bench::e2e-check` validates it without timing; formal and diagnostic workflows are documented in [../benchmarks/README.md](../benchmarks/README.md).

When changing public behavior, APIs, CLI flags, formats, operational workflows, or acceptance gates, update the matching active source and regenerate derived documentation in the same change.
