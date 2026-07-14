# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The selected `.stim` and `.dem` domains are complete at exact upstream-symbol and exported-Rust-API granularity. This checkpoint does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Clean evidence revision: `f62cdf135f6804419809e74d9e68c66551adf6e3` with `local_modifications=false`.

Correctness inventory digest: `cd82f99bd3f02446187b55c873e396e08c234f86693ba5f5734882fbe4814b56`.

Dependent performance inventory digest: `2cd3f0cd44f2450297dc87abec98156eb717127483a051f8dd48ee29bdc39fcd`.

## Delivered Slices

### `.stim` Format

- Twenty-four qualification parents map 44 exact upstream owners and nine exported-API owners to independently selectable tests; eight exact oracle-fixture parents remain direct evidence, producing 32 implemented `.stim` evidence parents and zero planned owners.
- Two `CQ-CIRCUIT-API` instruction parents map two exact Python semantic cases and four exact Rust methods without claiming the `CircuitInstruction` derived-trait owner or `Circuit::count_measurements`; those broader contracts remain planned in their owning domain.
- Semantic review found and fixed typed `CORRELATED_ERROR` and `ELSE_CORRELATED_ERROR` construction accepting inverted Pauli targets that Stim rejects.

### `.dem` Format

- Seventeen qualification parents, eight direct oracle-fixture parents, and three blocker-ledger parents produce 28 implemented `.dem` evidence parents and zero planned owners.
- The qualification parents cover target value and parsing, instruction values and validation, separator groups, parse and canonical print, tags and CRLF handling, mutation and repeats, counts and shifts, coordinate lookup, flattened traversal, materialization, and compact transforms.
- All 71 relevant pinned-upstream records and 128 selected exported Rust API items have exact ownership or an honest non-executable disposition. Python-only object shape, copying, indexing, operators, and file helpers remain deferred with Python bindings; C++ convenience operators and file helpers are not selected Rust compatibility contracts.
- Broad imported DEM fixture rows remain supporting provenance and are claimed through `oracle_fixture_owners`; they do not become atomic evidence or share a terminal exact primary with their qualification parent.
- Semantic review found and fixed a product bug: public `DemTarget::from_str` accepted standalone numeric tokens even though Stim accepts those tokens only in the internal `shift_detectors` instruction grammar.
- Mixed upstream symbols containing unimplemented `approx_equals`, moved-from C++ behavior, or search-only shortest-graphlike behavior were split instead of granting whole-file DEM-format credit.

The dependent PQ0 inventory was regenerated because correctness owner ids and acceptance state changed. No performance disposition, threshold, waiver, or comparator classification was relaxed.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 32 | 0 | 32 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 3 | 211 | 214 |
| `CQ-GATE-CONTRACT` | 84 | 646 | 730 |
| `CQ-BIT-KERNELS` | 4 | 384 | 388 |
| `CQ-CIRCUIT-API` | 37 | 325 | 362 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **204** | **2,276** | **2,480** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 303 | 303 | 0 | `target/qualification/correctness/cq2-dem-format-pr` | `f9471fc34bf86d2db4007fbf56024d10b2aa82345bf2441755a3fac06da9a0dd` | `a5282728cea8dbefa18f8b40476a7f19505b213de146d23857216c12ce292506` |
| Full | 457 | 457 | 0 | `target/qualification/correctness/cq2-dem-format-full` | `4d67f4b6ed6fd46a9db76f89f89f57690701727e0a79c92b6ace177f3c9717f9` | `6341f3e6e2f8921b8a6d77bf47e4a8bad87cb15aef826b0b5f4ab49e954001bf` |
| Soak | 457 | 457 | 0 | `target/qualification/correctness/cq2-dem-format-soak` | `8d3cc2695df3392c66522dc9e233baee2865dcf4be345af3164dddbd6c045b54` | `017cbac7824ca2c022ca696645d87425c0dbb630bf88a8e028b85b102eb3425d` |

Offline report regeneration passed for every tier. Exact full and soak preflight passed for `cq-evidence-qualification-0908c21b917526e3`, the canonical `.dem` parse, print, tag, and newline parent, against the controller-approved request and completion receipts.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Host | Artifact |
| --- | ---: | ---: | --- | ---: | --- | --- |
| PR | 3 | 1.015166 | [1.014561, 1.015270] | 0.000102 | Verified | `target/benchmarks/qualification/pq1-cq2-dem-doc-sync-pr` |
| Full | 9 | 1.015492 | [1.014614, 1.016164] | 0.000482 | Verified | `target/benchmarks/qualification/pq1-cq2-dem-doc-sync-full` |
| Soak | 15 | 1.015130 | [1.014543, 1.015421] | 0.000546 | Verified | `target/benchmarks/qualification/pq1-cq2-dem-doc-sync-soak` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `389a1cc7e3227c30485e14d8c3ee95315150e6b7`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. These ratios describe only the synthetic adapter protocol and must not be reported as Stab product performance.

## Verification

Passing checks for this checkpoint:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_dem_format --quiet
cargo test -p stab-oracle --quiet
cargo test -p stab-bench --quiet
just qualification::correctness-regenerate --check
just qualification::correctness-check
just bench::qualification-regenerate --check
just bench::qualification-check
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-pr
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-full
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-soak
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-pr
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-full
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-dem-doc-sync-soak
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, exact-preflight, and dependent PQ1 artifacts are identified above.

## Remaining Blocker

CQ2 still has 2,276 planned evidence owners. The active blocker is `CQ-RESULT-FORMATS`, where 211 of 214 owners remain planned across the `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` reader, writer, streaming, conversion, malformed-input, padding, width, and resource contracts. Continue with exact result-format semantic review before gates, bit kernels, circuit APIs, generation, and algebra.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until all selected CQ2 domains are implemented.
