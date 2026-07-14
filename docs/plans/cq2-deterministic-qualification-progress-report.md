# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The first source-owned slice establishes exact-parent mapping infrastructure and promotes reviewed `.stim` format contracts. It does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Clean evidence revision: `add37ccb6dc52b0ac96b37397f6b012de0bcd6a4` with `local_modifications=false`.

Correctness inventory digest: `5d1fc9d21e511e13bef5ceb476dbcf9dd20ed067339edd2891013992fb06ced5`.

Dependent performance inventory digest: `a7177e298b5e1f05979b871704514fdf2650070a7c48e5d72c6fb48bb80d13bf`.

## Delivered Slice

- `oracle/qualification-cases.json` is a bounded source-owned exact-parent ledger pinned to the selected Stim version and commit.
- Deterministic generation replaces only matching planned owners and rejects stale or missing owners, duplicate claims, cross-feature claims, comparator mismatches, non-exact Cargo selectors, unregistered property targets, and shared terminal primaries.
- Nineteen `.stim` format parents map 41 exact upstream or exported-API owners to focused existing tests, reducing total evidence owners by 22 without deleting provenance.
- `MeasureRecordOffset::{try_new,get}` has a focused positive, lower-bound, zero, positive, and overflow regression instead of receiving credit from a mixed typed-boundary test.
- The dependent PQ0 inventory was regenerated because correctness owner IDs and acceptance state changed; no performance disposition, threshold, or waiver was relaxed.
- Full-suite verification exposed and fixed a separate PQ1 process-runner race where a child-created thread could retain the broad CPU mask after the leader was pinned. The runner now bounds, pins, and verifies all existing child tasks before measured work.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 22 | 51 | 73 |
| `CQ-DEM-FORMAT` | 12 | 134 | 146 |
| `CQ-RESULT-FORMATS` | 3 | 211 | 214 |
| `CQ-GATE-CONTRACT` | 84 | 644 | 728 |
| `CQ-BIT-KERNELS` | 4 | 384 | 388 |
| `CQ-CIRCUIT-API` | 35 | 329 | 364 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **176** | **2,463** | **2,639** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 300 | 300 | 0 | `target/qualification/correctness/cq2-parent-refresh-pr` | `39fc89ae32b9670660225158aa093b8f8126958cbdd16ed61f7a3269704adbb8` | `06dd1af1b8515a533fb7409b9750a675e177db0a19a695463bb912b426e9041f` |
| Full | 429 | 429 | 0 | `target/qualification/correctness/cq2-parent-refresh-full` | `f131b4624e80d0855811d888f70eb16a9dde054a53db92146736329f9b1e0db9` | `50cfae396b074d391702435dfd856f7e4d3b70a66e30d6674515b419a1345e72` |
| Soak | 429 | 429 | 0 | `target/qualification/correctness/cq2-parent-refresh-soak` | `98b4799aebc9f3b690306fed3b0e414d663f2433a3e9c5d14d6329cd1919a683` | `e7f16974375890ed2b00f45ad595f2f234f835dfb15ea9999610bd389dba5cbb` |

Offline report regeneration passed for every tier. Exact preflight for `cq-evidence-qualification-e660819ae9a223c6`, the canonical `.stim` round-trip parent, passed against full and soak receipts. Its PR preflight was intentionally rejected because the source-owned case tiers are full and soak, so the case is correctly absent from the PR selection.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Host | Artifact |
| --- | ---: | ---: | --- | --- | --- |
| PR | 3 | 1.015911 | [1.014229, 1.017826] | Verified | `target/benchmarks/qualification/pq1-cq2-refresh-pr` |
| Full | 9 | 1.016132 | [1.015258, 1.016420] | Verified | `target/benchmarks/qualification/pq1-cq2-refresh-full` |
| Soak | 15 | 1.015357 | [1.014829, 1.016147] | Verified | `target/benchmarks/qualification/pq1-cq2-refresh-soak` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `add37ccb6dc52b0ac96b37397f6b012de0bcd6a4`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. These ratios must not be reported as Stab product performance.

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

CQ2 still has 2,463 planned evidence owners. The next work must continue semantic parent review and add focused tests where current selectors do not prove complete contracts, starting with the remaining `.stim` format owners and then `.dem`, result formats, gates, bit kernels, circuit APIs, generation, and algebra.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until the selected CQ2 domains are implemented rather than after this bootstrap slice.
