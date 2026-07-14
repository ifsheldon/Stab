# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The first source-owned deterministic slice now closes the selected `.stim` format domain and two exact `CircuitInstruction` semantic parents. It does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Clean evidence revision: `7420a6909dd998b721cd15075361f31e431e4180` with `local_modifications=false`.

Correctness inventory digest: `1152e9fa36d55c8a5a2282638fbc8ad79a39e7b4456161fba868a15c45cfb16e`.

Dependent performance inventory digest: `3b762ed4bcd44157fb5c7410912a30fb6fc7abb4ff69ab95a38ef9892c72bec6`.

## Delivered Slice

- `oracle/qualification-cases.json` is a bounded source-owned exact-parent ledger pinned to the selected Stim version and commit.
- Deterministic generation replaces only matching planned owners and rejects stale or missing owners, duplicate claims, cross-feature claims, comparator mismatches, non-exact Cargo selectors, unregistered property targets, and shared terminal primaries.
- Twenty-four `.stim` format qualification parents map 44 exact upstream owners and nine exported-API owners to independently selectable tests; eight exact oracle-fixture parents remain direct evidence, producing 32 implemented `.stim` evidence parents and zero planned owners.
- Two `CQ-CIRCUIT-API` instruction parents map two exact Python semantic cases and four exact Rust methods without claiming the `CircuitInstruction` derived-trait owner or `Circuit::count_measurements`; those broader API contracts remain planned.
- `MeasureRecordOffset::{try_new,get}` has a focused positive, lower-bound, zero, positive, and overflow regression instead of receiving credit from a mixed typed-boundary test.
- Semantic review found and fixed a product bug: typed `CORRELATED_ERROR` and `ELSE_CORRELATED_ERROR` construction now rejects inverted Pauli targets like Stim.
- The review also split mixed `circuit.test.cc`, gate-target equality, instruction value/count, and Python-only instruction-constructor ownership by exact symbol instead of inheriting a whole-file classification.
- The dependent PQ0 inventory was regenerated because correctness owner IDs and acceptance state changed; no performance disposition, threshold, or waiver was relaxed.
- Full-suite verification exposed and fixed a separate PQ1 process-runner race where a child-created thread could retain the broad CPU mask after the leader was pinned. The runner now bounds, pins, and verifies all existing child tasks before measured work.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 32 | 0 | 32 |
| `CQ-DEM-FORMAT` | 12 | 134 | 146 |
| `CQ-RESULT-FORMATS` | 3 | 211 | 214 |
| `CQ-GATE-CONTRACT` | 84 | 646 | 730 |
| `CQ-BIT-KERNELS` | 4 | 384 | 388 |
| `CQ-CIRCUIT-API` | 37 | 325 | 362 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **188** | **2,410** | **2,598** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 302 | 302 | 0 | `target/qualification/correctness/cq2-stim-format-docs-pr` | `085e11619fc16c1e88b4cdb1cf8d98b92b1023060dfb1dd5fa0e1ba80e2fe251` | `0816d26528c17dc8f181fddd812f41b8d458329c6e45c3c9c44b7d98e7664946` |
| Full | 441 | 441 | 0 | `target/qualification/correctness/cq2-stim-format-docs-full` | `293f5d1009718b58d5eb38b640f9ebcbfa3a89b00bb3817b2c04290894ef2917` | `57335dfb8b75f38bccc3d7d8af01e526d840c10a63eb379dd537e20fa7647cd7` |
| Soak | 441 | 441 | 0 | `target/qualification/correctness/cq2-stim-format-docs-soak` | `ff6e916925146673a2c0eedf808db2070362895b9c1b1e3a2c17604373ced49c` | `c5ea28aac1f1879e7fa967db36f6161dc92822c7b86360880ed6807683134dbd` |

Offline report regeneration passed for every tier. Exact preflight for `cq-evidence-qualification-e660819ae9a223c6`, the canonical `.stim` round-trip parent, passed against full and soak receipts. Its PR preflight was intentionally rejected because the source-owned case tiers are full and soak, so the case is correctly absent from the PR selection.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Host | Artifact |
| --- | ---: | ---: | --- | --- | --- |
| PR | 3 | 1.015929 | [1.014152, 1.017463] | Verified | `target/benchmarks/qualification/pq1-cq2-stim-format-docs-pr` |
| Full | 9 | 1.015046 | [1.014183, 1.015490] | Verified | `target/benchmarks/qualification/pq1-cq2-stim-format-docs-full` |
| Soak | 15 | 1.014783 | [1.014632, 1.015155] | Verified | `target/benchmarks/qualification/pq1-cq2-stim-format-docs-soak` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `7420a6909dd998b721cd15075361f31e431e4180`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. These ratios must not be reported as Stab product performance.

## Verification

Passing checks for this slice:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-oracle qualification --quiet
cargo test -p stab-bench --quiet
just qualification::correctness-regenerate --check
just qualification::correctness-check
just bench::qualification-regenerate --check
just bench::qualification-check
just bench::smoke
just maintenance::pre-commit
```

The clean PR, full, soak, report, preflight, PQ1 report, and PQ1 regression commands are represented by the artifact tables above.

## Remaining Blocker

CQ2 still has 2,410 planned evidence owners. The next work must continue semantic parent review and add focused tests where current selectors do not prove complete contracts, starting with `.dem` format and then result formats, gates, bit kernels, circuit APIs, generation, and algebra. Gate-target ordering and full circuit measurement-count semantics remain visible in their owning domains instead of being credited to the completed `.stim` slice.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until the selected CQ2 domains are implemented rather than after this bootstrap slice.
