# Instructions for Ops Tools

- The `ops/` tree holds the Rust binaries (`stab-oracle`, `stab-bench`, `stab-pre-commit`) that carry the repository's complex operational logic; recipes under `justfiles/` stay thin and call these binaries. Keep that split when adding or changing workflows.
- These tools handle hostile inputs, filesystem descriptors, bounded subprocess I/O, and atomic artifact publication. Preserve the fail-closed posture and do not reintroduce path-check-then-open fallbacks.
- Linux-only behavior in the qualification runtime is intentional: the evidence contracts require process-group termination, procfs, and atomic directory exchange, so fail closed on other platforms instead of emulating partial support.

## Read `docs/AGENTS.md` Before

- Modifying `ops/oracle/`: the fixture runner, compatibility-matrix and blocker validators, and the recorder implement the oracle corpus workflows contracted there.
- Modifying the correctness-qualification paths in `ops/bench/` (CQ bindings, case receipts, report validation, and preflight): the CQ0 inventory and CQ1 execution, selector, and publication contracts are defined there.
- Touching the `oracle/` ledgers (`qualification-cases.json`, `qualification-manifest.json`, the fixture manifest, or the compatibility matrix) or the `justfiles/oracle.just` and `justfiles/qualification.just` recipes.
- Writing or updating plan and progress documents under `docs/plans/`.

## Read `benchmarks/AGENTS.md` Before

- Modifying the performance paths in `ops/bench/`: paired runs, calibration, host policy, probes, reports, regression, rollup, and completion receipts.
- Touching source-owned benchmark files under `benchmarks/` (the benchmark manifest, the qualification suite, thresholds, waivers, runtime groups, or baseline dispositions) or the `justfiles/bench.just` recipes.

## Hook Maintenance

- `ops/pre-commit/` enforces the instruction-document policy (every scanned `README.md` needs a colocated `AGENTS.md`, and every effective `AGENTS.md` source needs a `CLAUDE.md` symlink), staged Rust checks, and oversized-blob scans. Run `just maintenance::pre-commit` after changing any of these surfaces.
