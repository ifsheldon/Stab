# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The selected `.stim`, `.dem`, and result-format domains are complete at exact upstream-symbol and exported-Rust-API granularity. This checkpoint does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Clean result-format evidence revision: `2f7e456b43415303eef19c3d2850811fa9b2526d` with `local_modifications=false`.

Correctness inventory digest: `adcc3d19605e4fc7bd9e1b3f3373ddf38d81301430d891f062baaab0c6fdc8f6`.

Dependent performance inventory digest after checklist synchronization: `3b55089bc331b115912818ba04b584c741a8c9a56e13d8b06d85cc829b6256a1`.

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

### Result Formats

- Thirty-six exact qualification parents and three independently selected direct oracle-fixture parents produce 39 implemented result-format evidence parents and zero planned owners.
- The ownership partition assigns all 156 selected upstream records and all 97 selected exported Rust API items exactly once. Forty-eight API-owning parents map the 97 API items without pretending that every trait method requires a duplicate terminal selector.
- Seven broad imported coverage fixtures remain typed supporting provenance, while `m7-convert-01-to-dets`, `m7-convert-bits-to-dets-reject`, and `pf3-m2d-text-format-negative-cli` retain direct independently selected ownership.
- Exact core tests cover `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` writers, readers, packed and sparse visitors, record and batch state, reference samples, 64/128/256/504/2048-bit widths, a 576-by-1,000 large table, malformed input, cancellation, padding, 64-shot groups, and resource boundaries.
- Exact CLI matrices cover explicit, circuit-derived, and DEM-derived layouts; detector and observable namespace separation; observable side output; raw widths; all accepted format round trips; and the 2,048-bit packed path.
- Semantic review found and fixed five contract defects: repeated sparse tokens now toggle dense parity, sparse visitors preserve source order and duplicates, scalar and batch lookback enforce the configured limit, batch intermediate writes no longer duplicate or flush incomplete 256-measurement chunks, and batch reference samples use global measurement indexes with Stim-compatible zero padding.

The dependent PQ0 inventory was regenerated because correctness owner ids and acceptance state changed. No performance disposition, threshold, waiver, or comparator classification was relaxed.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 32 | 0 | 32 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 84 | 646 | 730 |
| `CQ-BIT-KERNELS` | 4 | 384 | 388 |
| `CQ-CIRCUIT-API` | 37 | 325 | 362 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **240** | **2,065** | **2,305** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 31 | 31 | 0 | `target/qualification/correctness/cq2-result-formats-pr` | `fd6670d0a54af3eb8e071f0d796e2421d542ba0fd817944095825438dc4d8382` | `be4cfd4b117ffe8dec0ac6c1cff2edc84d90d2456fb8b5df12cb95c583b54a3b` |
| Full | 39 | 39 | 0 | `target/qualification/correctness/cq2-result-formats-full` | `d2f1fa31f29a3c236330a5e5fea58ea391657fed02a5b722dd5fa1dde856cb28` | `d9543566f5fc9648dbe0baf9acd510a7a5e5d2f587adbb6c4cfe3fec80a4b614` |
| Soak | 39 | 39 | 0 | `target/qualification/correctness/cq2-result-formats-soak` | `23fccd1aebc8d5e030896a724a63564db5364cfc13af94f183120e5ff07e5118` | `7282928fa577afe7b26573ce4e0ec9164d700c0c51258d7148f42b8d14455f29` |

Offline report regeneration passed for every tier. Exact full preflight passed for `cq-evidence-qualification-db7d4cd87fe69099`, the multi-width result-reader round-trip parent, against the controller-approved request and completion receipts.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds. The table below preserves the last clean pre-result-format evidence; a current `3b55089...` digest refresh is required before it can be cited as current harness evidence.

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
cargo test -p stab-core --test cq2_result_io --test cq2_result_formats --quiet
cargo test -p stab-cli convert --quiet
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

CQ2 still has 2,065 planned evidence owners. The active blocker is `CQ-GATE-CONTRACT`, where 646 of 730 owners remain planned across metadata, target validation, decompositions, tableaus, unitaries, aliases, inverse relations, execution semantics, malformed input, and resource contracts. Continue with an exact gate-family ownership partition before bit kernels, circuit APIs, generation, and algebra.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until all selected CQ2 domains are implemented.
