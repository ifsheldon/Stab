# M3 Completion Report

## Milestone

M3: Benchmark Baseline And Performance Contracts.

Objective: measure the pinned C++ baseline and freeze benchmark contracts before Rust implementations start optimizing against vague targets.

## Status

Complete for benchmark infrastructure and performance-contract definition.

M3 establishes `stab-bench`, benchmark manifests, pinned Stim baseline recording, Stab comparison report generation, generated-artifact output, strict compare validation, benchmark listing, and benchmark smoke wiring.
Implementation milestones M4 through M12 own the specific feature benchmark rows and performance evidence for their surfaces.

## Tests And Benchmarks Ported Or Created

- Added `benchmarks/manifest.csv` as the machine-readable benchmark contract matrix for parser/printer, gate lookup, bit kernels, algebra, generator, sampler, detector, analyzer, DEM, DEM sampler, and M12 primary performance gates.
- Added `ops/bench` as the Rust operational binary behind `just bench::smoke`, `just bench::list`, `just bench::baseline`, `just bench::compare`, and M12 primary gates.
- Added pinned Stim baseline report generation with machine metadata, command metadata, Stim tag, and Stim commit metadata.
- Added Stab comparison report generation with strict missing-baseline, invalid-baseline, pending-runner, and metadata checks.
- Added focused benchmark runner tests including `cargo test -p stab-bench --quiet`.

## Implementation Areas

- `ops/bench/src/main.rs` defines the benchmark CLI.
- `ops/bench/src/baseline.rs` and its milestone modules own pinned Stim baseline selection and Stab runner dispatch.
- `ops/bench/src/compare.rs` owns baseline validation, Stab measurement aggregation, strict compare behavior, and report generation.
- `ops/bench/src/report.rs` owns machine-readable JSON and Markdown report rendering.
- `benchmarks/manifest.csv` records the source-owned benchmark contract rows.
- `justfiles/bench.just` exposes the thin benchmark command surface.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Create `stab-bench` or equivalent benchmark package plus ops support | Satisfied | `ops/bench`; `Cargo.toml` workspace member `ops/bench`; package `stab-bench` |
| Add `just bench::baseline` to compile and benchmark pinned C++ Stim | Satisfied | `just bench::baseline --out target/benchmarks/m3-full-baseline --target-seconds 0.01 --cli-iterations 1` wrote a full baseline report for pinned Stim v1.16.0 |
| Add `just bench::compare` with baseline validation and strict selected-row behavior | Satisfied | `just bench::compare --baseline target/benchmarks/m3-full-baseline/baseline.json --report target/benchmarks/m3-full-compare` compared all 73 rows; `just bench::compare --milestone M4 --baseline target/benchmarks/m3-contract-baseline/baseline.json --strict --report target/benchmarks/m3-contract-compare` passed as focused strict selected-row evidence |
| Store benchmark results in machine-readable files under documented generated-artifact directories | Satisfied | `target/benchmarks/m3-full-baseline/baseline.json`; `target/benchmarks/m3-full-compare/compare.json`; generated Markdown reports beside each JSON file |
| Define benchmark contracts for parser/printer, `gen`, tableau operations, sampling, detection, analyzer, DEM, and DEM sampling | Satisfied | `benchmarks/manifest.csv`; `just bench::list` |
| Separate startup time, compile or analysis time, single-shot latency, and batch throughput where meaningful | Satisfied | `phase` and `measurement` columns in `benchmarks/manifest.csv`; `just bench::list` grouping |
| `just bench::baseline --stim vendor/stim` records pinned C++ baseline results for runnable rows and reports contract-only rows explicitly | Satisfied | Full M3 baseline recorded 73 rows against Stim v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, with 68 measured rows and 5 explicit contract-only rows |
| `just bench::list` prints every planned benchmark with owning milestone and threshold class | Satisfied | Command printed 73 planned rows grouped by milestone, runner, and threshold class |
| `just bench::smoke` runs in CI without requiring long benchmark durations | Satisfied | `.github/workflows/ci.yml`; `just bench::smoke` |
| Every implementation milestone M4 through M12 has named benchmark targets or explicitly says no benchmark is required | Satisfied | `docs/plans/rust-stim-drop-in-rewrite.md`; `benchmarks/manifest.csv`; `just bench::list` |

## Milestone Audit Outcome

- M3 does not claim feature performance parity by itself; it creates the benchmark contract and tooling that later milestones use.
- Contract-only rows are accepted only when they are explicit and cannot be mistaken for a measured pinned-Stim ratio.
- The 2026-06-28 GPT-5.5/xhigh milestone-audit pass initially found that this report used an M4-only focused baseline as M3 evidence.
- The missing M3 evidence was fixed by recording a full benchmark baseline and full non-strict compare report for all 73 rows.
- No open M3 under-specification entries remain in `docs/plans/milestone-spec-gaps.md`.

## Full Code Review Outcome

- The 2026-06-28 GPT-5.5/xhigh full-code-review pass found no blocking M3 documentation, benchmark-workflow, or traceability issues after the full-baseline correction.

## Verification Commands

- `just bench::smoke`
- `just bench::list`
- `just bench::baseline --out target/benchmarks/m3-full-baseline --target-seconds 0.01 --cli-iterations 1`
- `just bench::compare --baseline target/benchmarks/m3-full-baseline/baseline.json --report target/benchmarks/m3-full-compare`
- `just bench::compare --milestone M4 --baseline target/benchmarks/m3-contract-baseline/baseline.json --strict --report target/benchmarks/m3-contract-compare`
- `cargo test -p stab-bench --quiet`
