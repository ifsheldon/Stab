# Stab

## Development

This workspace is pinned to Rust Nightly `nightly-2026-06-20` in `rust-toolchain.toml`.
Nightly is required by the roadmap because the core bit kernels will use `std::simd` through `portable_simd`; until those kernels exist, the pin keeps local and CI behavior aligned.

Install the local staged-aware Git hook with:

```sh
just maintenance::setup-hooks
```

Run the same checks manually with:

```sh
just maintenance::pre-commit
```

The hook reads staged Git index entries, treats `vendor/stim` as a submodule pointer, runs Rust formatting and Clippy only for staged Rust-affecting changes, scans staged source blobs for oversized files, and checks instruction-document structure when `README.md`, `AGENTS.md`, `CLAUDE.md`, or `.gitmodules` changes.
Every scanned `README.md` needs a colocated `AGENTS.md`, and every effective `AGENTS.md` source needs at least one `CLAUDE.md` symlink pointing to it.

Generate Rust API documentation with:

```sh
just docs::api
```

The generated Rust API reference is written under `target/doc/`, including `target/doc/stab_core/index.html`.
Run the stricter documentation check before changing public Rust APIs:

```sh
just docs::api-check
```

This check runs rustdoc for the workspace with warnings denied.

Validate the pinned Stim oracle with:

```sh
just oracle::version
```

Run the M0 oracle smoke cases with:

```sh
just oracle::run --case smoke/help
just oracle::run --case smoke/tiny-circuit
```

The tiny-circuit smoke case now runs through the public `stab sample` command.
M7 early CLI compatibility includes `stab gen` for `repetition_code memory`, `surface_code rotated_memory_x`, `surface_code rotated_memory_z`, `surface_code unrotated_memory_x`, `surface_code unrotated_memory_z`, and `color_code memory_xyz`.
The supported `gen` flags are `--code`, `--task`, `--distance`, `--rounds`, `--after_clifford_depolarization`, `--after_reset_flip_probability`, `--before_measure_flip_probability`, `--before_round_data_depolarization`, `--out`, and Stim's accepted no-op `--in`; the deprecated `--gen` spelling is accepted for Stim v1.16.0 compatibility.
`stab convert --in_format=stim --out_format=stim` reads from stdin or `--in`, writes to stdout or `--out`, and emits canonical `.stim` text through the M4 parser/printer.
The result-data conversion surface supports `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` input and output formats with layouts from explicit `--num_measurements`, `--num_detectors`, and `--num_observables` counts, `--dem`, or `--circuit` plus unique `--types` letters from `M`, `D`, and `L`.
`convert` also supports `--obs_out`, `--obs_out_format`, the legacy top-level `--convert` alias, default `--in_format=01`, and 64-record grouping checks for `ptb64` output.
`convert --bits_per_shot ... --out_format=dets` is rejected like Stim because raw bit width does not identify measurement, detector, or observable record types.
`stab help [topic]` and top-level `stab --help [topic]` provide Stab-native structural help for implemented commands, result formats, and gate names without claiming byte-for-byte Stim help text.
M8 sampling compatibility is done for the selected Rust and CLI surface, including deterministic `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` output, count-determined measurement and reference-sample-tree helpers, basis, pair, and Pauli-product measurements, feedback, entangling Clifford sampling, repeat handling, and seeded supported noise channels. Exact random-stream parity and public interactive simulator products remain deferred.
Compiled samplers expose heralded-noise measurement records. For Stim v1.16.0 CLI compatibility, `stab sample` omits those columns only on the default one-shot tableau path; multi-shot and `--skip_reference_sample` paths emit them.
M9 detection compatibility includes public `stab detect` and `stab m2d` rows for deterministic detector conversion, observable routing, reference-sample subtraction, text and bit-packed detection records, and M9 benchmark compare runners.
M10 DEM compatibility is done for the selected Rust and CLI analyzer surface, including `.dem` parsing and canonical printing, supported noise and measurement analysis, gauge handling, decomposition controls, generated and folded-loop cases, graphlike and hypergraph search, SAT/WCNF output, sparse reverse tracking, and matched-error values. Full ErrorMatcher provenance and `explain_errors` remain deferred.
M11/M12 DEM sampling compatibility includes deterministic and statistical sampling, detector and observable streams, sampled-error output and replay, supported result formats, bounded parsing, and streaming CLI writers backed by reusable visitor APIs. Materialized APIs and inherently expanded sampled-error records retain documented caps; exact Python API shape remains deferred.

Inspect and check the M2 fixture corpus with:

```sh
just oracle::list
just oracle::list --milestone M4
just oracle::blockers
just oracle::blockers --list
just oracle::blockers --check-selectors
just oracle::record --check-clean
just oracle::run --implemented-only
just oracle::run --milestone M4
just oracle::run --all
```

The fixture manifest lives at `oracle/fixtures/manifest.csv`.
It records fixture ids, upstream sources, command shapes, parity modes, comparator types, expected statuses, implementation status, statistical plans, and source-license notes.
Manifest validation also requires every planned M4 through M11 P0/P1 C++ source from the compatibility matrix to have an explicit fixture row with the matching milestone and parity mode.
Manifest `argv` tokens may use `{fixture_input:inputs/name.ext}` for validated fixture-relative side inputs and `{fixture_output:expected/name.ext}` for exact-output side files written by Stim and Stab during comparison.
Both placeholder forms reject absolute paths, parent-directory traversal, symlinks in fixture paths, and missing fixture inputs; fixture-output placeholders require committed expected files during normal validation and `just oracle::record --check-clean` unless the row is a statistical fixture with `source=fixture_output`.
Statistical fixture rows default to `source=stdout`; `source=fixture_output` requires exactly one `{fixture_output:...}` placeholder, uses that fixture-relative path as a validated scratch label, exact-compares stdout and stderr against pinned Stim, and applies the row's statistical plan to both pinned Stim and Stab side-output bytes.
On Linux oracle runs, fixture-output placeholders are rewritten to inherited `/proc/self/fd/...` paths backed by a fresh private directory under `/tmp`; the controller monitors and reads each side output relative to the retained directory descriptor with `NOFOLLOW`, performs bounded descriptor-relative cleanup, and compares exact-output row side-output bytes in addition to stdout.
`just oracle::run --milestone Mx` scopes execution to implemented fixture rows for that milestone and reports pending red, ignored, or manifest-only rows in the same milestone.
`just oracle::record --check-clean` checks runnable exact-output rows against pinned Stim; library-only parser/printer rows are run in-process by `stab-oracle` and are skipped by recording because they do not have a Stim CLI command.
`just oracle::blockers` validates the schema-versioned blocker closure ledger against the pinned Stim tag and commit, tracked regular upstream source files, exact test and symbol anchors, planned test-family anchors, reproducible statistical plans, required PFM-B owners, test evidence state, implemented primary and supporting oracle rows, typed oracle and benchmark runners, benchmark comparability classes, resource contracts, resource limits, and the frozen SHA-256 semantic inventory.
`just oracle::blockers --check-selectors` additionally runs allowlisted `cargo test ... -- --list` commands through timed, bounded process capture and rejects claimed existing selectors that match no tests.
The blocker-ledger validator currently requires Unix file-identity support and fails closed on other targets instead of accepting a symlink race.

Validate the M1 compatibility matrix with:

```sh
just oracle::matrix --check
just oracle::matrix --milestone M4
```

The matrix lives at `oracle/compatibility-matrix.csv` and records upstream source paths, owners, milestones, priorities, parity modes, comparators, status, acceptance checks, and deferred future buckets.

The active follow-up plans are [comprehensive correctness qualification](docs/plans/comprehensive-correctness-qualification-plan.md) and [comprehensive Stim performance qualification](docs/plans/comprehensive-stim-performance-qualification-plan.md), with execution rules in [GOAL.md](docs/plans/GOAL.md).
CQ0 and PQ0 provide case-level correctness and feature-level performance disposition ledgers, and CQ1 provides independently selectable correctness execution, PR/full/soak tiers, manifest-bound reports, and machine-readable preflight evidence. Clean CQ1 acceptance results are recorded in [the CQ1 progress report](docs/plans/cq1-correctness-harness-progress-report.md).
PQ1 is complete at the clean evidence in [pq1-performance-harness-progress-report.md](docs/plans/pq1-performance-harness-progress-report.md). It provides the bounded symmetric process runner, pinned-Stim adapter, Stab worker, calibrated paired statistics, controlled-host and current-toolchain checks, process-memory evidence, atomic reports, and regression dispatch. Its synthetic protocol-smoke group validates infrastructure only and can never become a product performance claim; product workload qualification, scaling, and threshold graduation remain PQ2 through PQ7 work.

CQ0 inventory discovery is implemented through:

```sh
just qualification::correctness-list
just qualification::correctness-list --feature CQ-RESULT-FORMATS
just qualification::correctness-check
just qualification::correctness-regenerate --check
```

These commands validate or deterministically regenerate `oracle/qualification-manifest.json` from the pinned C++ and Python test tree, default-feature rustdoc JSON, current implemented oracle rows, and the reviewed exact-parent mappings in `oracle/qualification-cases.json`.
The qualification-case ledger may bind several exact upstream or exported-API owners to one independently selectable test only when they share one feature and comparator and that test proves the complete parent contract; regeneration rejects stale, duplicate, cross-feature, comparator-mismatched, and shared-primary mappings.
CQ1 correctness execution and report commands are implemented through:

```sh
just qualification::correctness-provenance-probe
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full
just qualification::correctness-run --tier soak
just qualification::correctness-report --out target/qualification/correctness/latest
just qualification::correctness-preflight --out target/qualification/correctness/latest --case <qualification-case-id> --request-sha256 <run-request-sha256> --completion-sha256 <run-completion-sha256>
```

Qualification runs require a pinned Stim checkout with no tracked or untracked modifications, build fresh private Release binaries for Stab and Stim with Cargo invoked from `/` using absolute manifest paths, execute exact source-owned selectors through immutable sealed copies of direct tools under a config-free explicit child environment, enforce bounded output with process-group cleanup, publish complete run directories atomically, and print the request, report, and completion digests needed by downstream controllers.
The provenance probe rebuilds both binaries, runs one real source-owned exact case through the normal qualification runner, and reads back the request, execution, report, completion, and preflight artifacts to prove their executable, environment, selector, output, and digest bindings agree.
Git metadata inspection uses a config-free private Git view whose index is reconstructed from `HEAD`, so caller index flags cannot hide tracked modifications; CMake modules and compiler include or support trees are copied into read-only content-bound snapshots, and compiler subordinate programs are sealed before the build; support-tree digests are included in the hashed execution environment and rechecked after every compiler-consuming case and before publication.
Private build scratch uses fixed private directories under `/tmp`, Cargo's working directory is `/` so source or scratch ancestors cannot contribute `.cargo` configuration, and retained parent and root descriptors drive bounded cleanup of read-only support snapshots without following replacement paths; an over-budget cleanup is quarantined instead of falling through to unbounded path removal.
`request.json` fixes the intended selection, canonical direct-executable role/hash/size ledger, and hashed execution environment before execution; each schema-version-3 case execution receipt fixes that same provenance together with process status, output framing, exact Cargo test count, exact statistical completion, and retained artifacts; and `completion.json` fixes the canonical report plus all case receipts after execution.
Fixed Cargo statistical selectors must emit one source-owned completion marker per declared comparison after its structurally valid shot batches finish and before probabilistic acceptance; each marker must contain exactly the frozen batches per comparison, a malformed suffix retains only the validated marker prefix, and two-sided fixtures retain exact completed-side work on a failed attempt without allowing a partial attempt to pass.
Staged evidence uses descriptor-relative artifact I/O, newly created receipt directories and their parents are synchronized before publication, publication is serialized by a lock anchored to the retained repository directory, cancellation is rechecked after acquiring that lock, and report regeneration reopens and identity-checks the complete output parent chain before accepting derived evidence.
Preflight requires controller-approved request and completion digests and rejects stale, partial, failed, deferred, modified, executable-mismatched, directory-swapped, or selector-mismatched evidence.
Dirty reports can be inspected with explicit `--allow-dirty` preflight permission but are never promotable completion evidence.
CQ1 execution currently fails closed outside Linux because its evidence contract requires Linux process-group termination and atomic directory exchange.
Promotable CQ1 evidence still assumes a controlled host: the outer `cargo run` bootstrap, Linux kernel, procfs, process and dynamic-loader semantics, system shared libraries, and dependency-cache contents remain in the trust root, while recorded SHA-256 identities provide reproducibility and tamper evidence rather than third-party authenticity.
Cargo cases consume the live checkout after request publication, so promotable runs also require that no concurrent same-UID process transiently mutates and restores source files, Git refs or objects, or support-tree aliases during the run; pre-run and pre-publication validation detects persistent changes but is not an authenticated defense against a malicious local operator.

PQ0 performance-ledger discovery is implemented through:

```sh
just bench::qualification-list
just bench::qualification-list --feature PERF-RESULT-IO
just bench::qualification-check
just bench::qualification-regenerate --check
```

These commands validate or deterministically regenerate `benchmarks/stim-qualification-suite.json` from the frozen CQ0 digest, Stab feature checklist, current benchmark manifest, primary thresholds and waivers, and all pinned upstream perf sources and symbols.
The PQ1 process and adapter contracts can be reproduced independently, and the paired controller can publish PR, full, or soak evidence through:

```sh
just bench::qualification-probe --group pq1-process-contract-smoke
just bench::qualification-probe --group pq1-adapter-protocol-smoke
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-pr
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-full
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-soak
just bench::qualification-report --input target/benchmarks/qualification/pq1-full
just bench::qualification-regression --input target/benchmarks/qualification/pq1-full
```

The controller validates `benchmarks/qualification-host-policy.json`, acquires an exclusive profile-and-CPU qualification lease before host capture or private builds, pins both workers to the selected CPU, requires stable thermal probes at or below 85000 millidegrees Celsius, targets 350-millisecond calibration batches with a 250-millisecond revalidation floor and a 2-second ceiling, retains three warmups and every interleaved pair, computes fixed-seed bootstrap intervals, records setup and peak RSS separately from timing, and atomically publishes `report.json`, `preflight.json`, and `report.md`. The lease remains held through final host capture, report validation, and atomic publication. The parent derives every expected work count, uses work-bound calibration probes to select one common batch shape, performs semantic preflight at that exact shape, binds every subsequent validation, warmup, sample, and memory digest to the preflight, and audits the revision through a config-free private Git view reconstructed from an exact captured commit. Each run materializes committed Stab and pinned-Stim source into private `/tmp` trees, performs fresh controlled Cargo and CMake release builds, records canonical tools, arguments, environments, source inputs, build fingerprints, and binary digests, and invokes both workers from sealed executable copies. Child stdin, stdout, stderr, runtime, process trees, and regular-file growth are bounded; file-writing qualification adapters must wait on the controller start barrier so the Linux file-size limit is installed before output begins.
Strict runs reject host-policy violations. `--allow-unverified-host` preserves local diagnostic evidence with `host_verified=false`, but that evidence cannot be promoted. Offline report refresh, preflight, and regression commands reload the current source-owned host policy and checked inventories, replay the current pinned Rust toolchain, require the same host CPU identity and affinity set, bind the report to its exact output directory and regenerated preflight, and reconstruct the exact violations and verification outcome from the recorded probes. Report refresh uses compare-and-swap publication so stale validation cannot overwrite newer evidence. The PQ1 protocol-smoke group is report-only in `benchmarks/qualification-baseline.json`, does not accept CQ evidence, and must not be cited as Stab-versus-Stim product speed evidence.
Offline validation also replays the source-owned calibration algorithm and requires every semantic preflight, common validation, warmup, retained sample, and memory invocation to use the exact workload, measurement, implementation, evidence mode, calibrated iteration count, derived work count, CPU, build identity, and required preflight output digest. Repeated memory summaries must reproduce the raw worker and parent-observed RSS receipts.
Noise classification uses paired-ratio relative MAD only. An initial attempt above the 10 percent noise limit triggers exactly one complete second timing attempt with three new warmups and the full retained pair count; both attempts remain in the report, and the second attempt is authoritative regardless of whether it passes, fails, or remains noisy. Regression evaluation rejects failed and noisy authoritative outcomes before applying numeric thresholds.
`benchmarks/qualification-runtime-groups.json` is the source of truth for executable runtime groups and binds each group to its claim class, baseline eligibility, worker workload, exact measurement set, correctness cases, and frozen performance inventory. `benchmarks/qualification-baseline.json` must contain one matching disposition for every runtime group; diagnostic groups require an explicit report-only entry with no thresholds, while threshold-eligible groups require an exact complete measurement-rule set. Future promotable groups take their CQ case IDs only from this ledger and require controller-approved CQ request and completion digests; report refresh independently reconstructs the canonical request, report, completion, preflight, and per-case execution receipts before accepting the performance report.
Stab allocation counts remain a separate Rust-only regression signal through `just bench::compare-allocations`; they are not mixed into paired timing or cross-implementation process-RSS claims.
PQ1 qualification execution is Linux-only because affinity, process groups, procfs RSS, regular-file limits, and atomic directory exchange are part of its evidence contract. Final product performance qualification targets controlled Linux x86-64 and Linux AArch64 hosts independently.

Benchmark contracts live at `benchmarks/manifest.csv`.
Each benchmark row owns its runner, threshold class, and comparability class; `just bench::list` prints all three, and manifest validation requires the comparability field to agree with the source-owned compare-note prefix.
The M3 benchmark workflow validates those contracts, records pinned C++ Stim baseline results, and writes generated reports under `target/benchmarks/`.
Any explicit `--out` path must be repository-relative and under `target/benchmarks/`.
`--only` filters use exact benchmark row ids or milestone names such as `M7` on both baseline and compare commands.

```sh
just bench::list
just bench::smoke
just bench::baseline --stim vendor/stim --primary
just bench::baseline --only m7-convert-01-to-b8 --out target/benchmarks/convert-probe-baseline
just bench::compare --milestone M4
just bench::compare --only m7-convert-01-to-b8 --baseline target/benchmarks/convert-probe-baseline/baseline.json --report target/benchmarks/convert-probe-compare
just bench::compare --profile release --primary --report target/benchmarks/latest --require-beta-gate --require-profiler-notes
just bench::compare-allocations --primary --report target/benchmarks/latest-allocations
```

`just bench::baseline` writes `baseline.json` and `report.md` to `target/benchmarks/baseline/latest` by default.
Use `--primary` when recording the M12 baseline consumed by `just bench::compare --primary`.
Use a small `--target-seconds` value for quick local smoke runs, and increase it when recording durable baseline artifacts.
`just bench::compare` runs the benchmark ops binary with Cargo's release profile, reads `target/benchmarks/baseline/latest/baseline.json` by default, and can write `compare.json` plus `report.md` to a repository-relative directory under `target/benchmarks/`.
Use `--require-beta-gate` for completion-style runs where every selected row must prove a Stab median no slower than 1.25x pinned Stim.
Profiler notes for rows slower than 1.5x pinned Stim live under the report directory's `profiler-notes/` folder and must include `Dominant cost:` and `Next owner action:` lines when `--require-profiler-notes` is used.
Use `--thresholds <path>` once regression thresholds exist to fail selected rows that exceed their configured maximum relative ratio or cannot produce a comparable ratio.
`just bench::primary-regression` checks the source-owned M12 threshold file with a Stab-side warmup pass and three recorded measurement runs; the scheduled M12 benchmark workflow runs it against a fresh primary pinned-Stim baseline and uploads the generated reports.
`just bench::compare-allocations` builds `stab-bench` with the optional `count-allocations` feature and adds Stab-side allocation counts plus resident-memory samples to the compare report; keep timing-gate runs on plain `just bench::compare` so allocation instrumentation does not affect timing evidence.
Use `--require-memory-gate --memory-baseline <compare.json>` with `just bench::compare-allocations` once the first complete memory report exists to fail rows whose peak live allocation bytes or sampled resident bytes exceed the M12 25 percent regression budget.
