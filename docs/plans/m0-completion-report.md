# M0 Completion Report

## Milestone

M0: Project Foundation And Oracle Lab.

Objective: make the repository reproducible, staged-checkable, and able to call the pinned Stim v1.16.0 oracle before any large feature work starts.

## Status

Complete against the M0 smoke-oracle contract.

M0 establishes the Cargo workspace, pinned nightly toolchain, thin `just` command surface, Rust operational binaries, pinned Stim v1.16.0 submodule checks, local pre-commit workflow, CI smoke gates, oracle smoke cases, and benchmark smoke wiring.
The exact `m0-help-exact` row remains an explicit red fixture from the M2 red-test contract; it is not an M0 blocker because the M0 done criteria require help-command health, binary entry health, and one tiny deterministic circuit smoke case instead of byte-for-byte CLI help parity.

## Tests Ported Or Created

- Added the `smoke-help` oracle row from `src/stim/main_namespaced.test.cc` using the `help-health` comparator.
- Added the `smoke-tiny-circuit` exact-output oracle row from `src/stim.test.cc` as the tiny deterministic `.stim` sample smoke case.
- Kept `m0-help-exact` as an explicit red exact-output row so byte-for-byte help parity stays visible without blocking the M0 smoke scope.
- Added staged pre-commit checks in the `stab-pre-commit` Rust ops binary, including Cargo formatting, clippy, tests, fixture manifest validation, matrix validation, docs and symlink checks, and large-file watch-list reporting.
- Added CI steps for `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `just oracle::version`, the two M0 oracle smoke cases, and `just bench::smoke`.

## Implementation Areas

- `Cargo.toml` defines the workspace members for `stab-core`, `stab-cli`, `stab-oracle`, `stab-bench`, and `stab-pre-commit`.
- `rust-toolchain.toml` pins the Rust nightly toolchain required by the `portable_simd` policy.
- `justfile` delegates to modular files under `justfiles/` for Rust checks, maintenance, oracle, and benchmark workflows.
- `ops/oracle` owns pinned Stim submodule validation, version checks, smoke comparison, compatibility matrix validation, fixture listing, and fixture recording.
- `ops/bench` owns benchmark manifest validation, pinned Stim baseline recording, and Stab-vs-Stim comparison report generation.
- `ops/pre-commit` owns staged repository checks without a tracked shell script.
- `.github/workflows/ci.yml` runs the M0 local smoke gates in CI.
- `vendor/stim` is pinned to tag `v1.16.0` at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Convert the repo into the planned Cargo workspace layout | Satisfied | `Cargo.toml`; `cargo metadata --no-deps --format-version 1` lists `stab-cli`, `stab-core`, `stab-bench`, `stab-oracle`, and `stab-pre-commit` |
| Pin nightly and document why nightly is required | Satisfied | `rust-toolchain.toml`; `docs/plans/rust-stim-drop-in-rewrite.md` portable-SIMD decision |
| Keep the root `justfile` thin and dispatch through modular justfiles | Satisfied | `justfile`; `justfiles/rust.just`; `justfiles/maintenance.just`; `justfiles/oracle.just`; `justfiles/bench.just` |
| Keep complex operational logic in Rust binaries | Satisfied | `ops/oracle`; `ops/bench`; `ops/pre-commit` |
| Add `just oracle::fetch`, `just oracle::version`, and `just oracle::run` | Satisfied | `justfiles/oracle.just`; `ops/oracle/src/main.rs` |
| Add CI jobs for formatting, linting, unit tests, oracle smoke tests, and benchmark smoke tests | Satisfied | `.github/workflows/ci.yml` |
| `just maintenance::setup-hooks` installs a working local pre-commit hook without a tracked shell script | Satisfied | `just maintenance::setup-hooks` installed `.git/hooks/pre-commit` from `target/debug/stab-pre-commit`; `cargo test -p stab-pre-commit --quiet` |
| `just oracle::version` fails unless `vendor/stim` resolves to the pinned v1.16.0 commit | Satisfied | `just oracle::version` reported expected and actual tag `v1.16.0`, expected and actual commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, and `Status: OK` |
| `just oracle::run --case smoke/help` passes | Satisfied | Command passed with status `Some(0)` and stderr class `Empty` |
| `just oracle::run --case smoke/tiny-circuit` passes | Satisfied | Command passed with status `Some(0)` and stderr class `Empty` |
| `just bench::smoke` runs as compile and wiring smoke only | Satisfied | Command reported `benchmark manifest OK: 73 planned rows` |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all --check` pass locally and in CI | Satisfied | `.github/workflows/ci.yml`; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all passed in the latest GOAL recheck |

## Milestone Audit Outcome

- M0 scope is intentionally smoke-level: exact CLI help parity, parser behavior, gate metadata, simulator behavior, and benchmark baselines are owned by later milestones.
- The 2026-06-28 GPT-5.5/xhigh milestone-audit pass initially found that this report omitted GOAL audit/review evidence and local M0 command evidence.
- The missing evidence was fixed by recording `just maintenance::setup-hooks`, workspace formatting, clippy, and test results in this report.
- No open M0 under-specification entries remain in `docs/plans/milestone-spec-gaps.md`.

## Full Code Review Outcome

- The 2026-06-28 GPT-5.5/xhigh full-code-review pass found no blocking M0 documentation, workflow, or implementation issues.
- The current watch-list risk is repository growth near the 1200-line large-file threshold; no reviewed Rust source file currently exceeds the threshold.

## Verification Commands

- `just maintenance::setup-hooks`
- `just oracle::version`
- `just oracle::run --case smoke/help`
- `just oracle::run --case smoke/tiny-circuit`
- `just bench::smoke`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p stab-pre-commit --quiet`
- `find . -path './target' -prune -o -path './.git' -prune -o -type f \( -name '*.rs' -o -name '*.md' -o -name '*.toml' \) -print0 | xargs -0 wc -l | sort -nr | head -40`
