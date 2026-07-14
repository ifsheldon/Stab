# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The selected `.stim`, `.dem`, result-format, and gate-contract domains are complete at exact upstream-symbol and exported-Rust-API granularity. This checkpoint does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Clean current-digest global evidence revision: `c6ca93945ad7e94e758e2cdea745a9b2a62c401a` with `local_modifications=false`.

Current correctness inventory digest after gate-contract reconciliation: `4ee9686104c0d537073a823986cb71b8bc092c7a9f09fbdbee75c4af7d2c6b70`.

Current dependent performance inventory digest: `9ae9b79c172c27f2a33475d856cca97c668c6608781cb81b8a8861f46cd13966`.

Clean current-digest CQ1 and PQ1 reports are published at the artifact paths below. The preceding result-format reports remain historical evidence and are superseded for current-digest claims.

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

### Gate Contract

- Thirty-seven implemented blocker-ledger parents, fourteen independently selected oracle-fixture parents, and eight focused qualification parents produce 59 implemented gate-contract evidence parents and zero planned owners.
- The exact partition assigns all 178 selected exported Rust API items. Of 540 relevant pinned-upstream records, 328 are ported through exact Rust ownership, 12 remain explicit semantic-mining references, and 200 are deferred with Python bindings or the public interactive graph and vector simulator products.
- Schema-version-2 `oracle/qualification-cases.json` mappings reuse canonical implemented or evidence-close blocker, oracle, and Rust-regression parents without duplicating terminal selectors. Explicit 64-bit, 128-bit, and 256-bit family declarations expand to exact upstream symbols and reject missing, duplicate, or unsupported word sizes.
- Focused qualification tests cover the 81-gate registry, exact name lookup and rejection, unitary and decomposition metadata, gate-target text and values, target accessors, inversion, and target classification. Canonical gate semantic tests cover deterministic and noisy measurement-only and measure-reset gates, pair and product measurements, Pauli channels, depolarization, correlated and heralded errors, annotations, classical controls, SPP families, rejection contracts, and reset postconditions.
- Nine broad imported gate fixtures remain supporting provenance on exact canonical parents instead of becoming atomic primary evidence. C++ storage-layout and helper-only tests are not applicable, Python object-shape behavior remains deferred with Python bindings, and public graph and vector simulator tests remain deferred with those explicitly excluded products.
- Review exposed two evidence loopholes and closed them in focused tests: noisy X/Y/Z evidence now exercises both measurement-only and measure-reset gates, and annotation evidence now exercises Pauli-target observable semantics in all three bases.
- The first clean PR execution correctly rejected those broadened noisy X/Y/Z tests because the blocker ledger and core plan still declared one 100,000-shot comparison while each selector emitted two. Both source-owned plans now declare two independent comparisons and two shot batches, the helper asserts that contract locally, and no controller relaxation was made.

The dependent PQ0 inventory was regenerated because correctness owner ids and acceptance state changed. No performance disposition, threshold, waiver, or comparator classification was relaxed.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 32 | 0 | 32 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 59 | 0 | 59 |
| `CQ-BIT-KERNELS` | 4 | 384 | 388 |
| `CQ-CIRCUIT-API` | 37 | 325 | 362 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **215** | **1,419** | **1,634** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 336 | 336 | 0 | `target/qualification/correctness/cq2-gate-global-pr` | `a3350e81f680102c2289005105e327c4d6c40c44a0e1941d2df83bf53141d18c` | `438cf888554700de5cfcff9b051e9ee8d2225c11f517d6e8881d25acb090f426` |
| Full | 501 | 501 | 0 | `target/qualification/correctness/cq2-gate-global-full` | `9ccd05621ad0d7c3592240bf0d4b358be1f91db3bd93b0dfc5ac5b552191fd11` | `54601a2ac5be94c31860644d6ce92a7e52e54bb4f821f8194e810787fa2a0873` |
| Soak | 501 | 501 | 0 | `target/qualification/correctness/cq2-gate-global-soak` | `37eb06d9522859b934f2d65bfa349f6e6db716fd0345deb66bf55011a1e0c973` | `9d8835546ea39c9ea02b6848a53adc8961f3317bb003a0305ec22c1ac039c35f` |

Offline report regeneration passed for every tier. Exact PR, full, and soak preflight passed for `cq-evidence-blocker-4b96f79267ebd43d`, the noisy X measurement and measure-reset parent, against the controller-approved request and completion receipts. PR and full each completed all 4,218,400 planned statistical shots with consumed false-positive bound `2.67062845963454362e-6`; soak completed all 4,847,200 shots with consumed bound `5.98047030092843113e-6`, below the declared suite budget `3.20000000000000053e-5`.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Host | Artifact |
| --- | ---: | ---: | --- | ---: | --- | --- |
| PR | 3 | 1.015311 | [1.014785, 1.015760] | 0.000442 | Verified | `target/benchmarks/qualification/pq1-cq2-gate-pr` |
| Full | 9 | 1.014834 | [1.012124, 1.015011] | 0.000183 | Verified | `target/benchmarks/qualification/pq1-cq2-gate-full` |
| Soak | 15 | 1.015483 | [1.014967, 1.016240] | 0.000746 | Verified | `target/benchmarks/qualification/pq1-cq2-gate-soak` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `c6ca93945ad7e94e758e2cdea745a9b2a62c401a`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. These ratios describe only the synthetic adapter protocol and must not be reported as Stab product performance.

## Verification

Passing checks for the current checkpoint:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_gate_contract --quiet
cargo test -p stab-core gate_surface_contract_measure_reset_ --quiet
cargo test -p stab-core gate_surface_contract_annotations --quiet
cargo test -p stab-core gate_surface_contract_ --quiet
cargo test -p stab-core --test gate_metadata --quiet
cargo test -p stab-core --test stim_format --quiet
cargo test -p stab-oracle blocker_ledger --quiet
cargo test -p stab-oracle qualification::statistics --quiet
cargo test -p stab-core --test cq2_dem_format --quiet
cargo test -p stab-core --test cq2_result_io --test cq2_result_formats --quiet
cargo test -p stab-cli convert --quiet
just qualification::correctness-regenerate --check
just qualification::correctness-check
just bench::qualification-regenerate --check
just bench::qualification-check
just qualification::correctness-run --tier pr --out target/qualification/correctness/cq2-gate-global-pr
just qualification::correctness-run --tier full --out target/qualification/correctness/cq2-gate-global-full
just qualification::correctness-run --tier soak --out target/qualification/correctness/cq2-gate-global-soak
just qualification::correctness-report --out target/qualification/correctness/cq2-gate-global-pr
just qualification::correctness-report --out target/qualification/correctness/cq2-gate-global-full
just qualification::correctness-report --out target/qualification/correctness/cq2-gate-global-soak
just qualification::correctness-preflight --out target/qualification/correctness/cq2-gate-global-pr --case cq-evidence-blocker-4b96f79267ebd43d --request-sha256 a3350e81f680102c2289005105e327c4d6c40c44a0e1941d2df83bf53141d18c --completion-sha256 438cf888554700de5cfcff9b051e9ee8d2225c11f517d6e8881d25acb090f426
just qualification::correctness-preflight --out target/qualification/correctness/cq2-gate-global-full --case cq-evidence-blocker-4b96f79267ebd43d --request-sha256 9ccd05621ad0d7c3592240bf0d4b358be1f91db3bd93b0dfc5ac5b552191fd11 --completion-sha256 54601a2ac5be94c31860644d6ce92a7e52e54bb4f821f8194e810787fa2a0873
just qualification::correctness-preflight --out target/qualification/correctness/cq2-gate-global-soak --case cq-evidence-blocker-4b96f79267ebd43d --request-sha256 37eb06d9522859b934f2d65bfa349f6e6db716fd0345deb66bf55011a1e0c973 --completion-sha256 9d8835546ea39c9ea02b6848a53adc8961f3317bb003a0305ec22c1ac039c35f
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-cq2-gate-pr
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-cq2-gate-full
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-cq2-gate-soak
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-gate-pr
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-gate-full
just bench::qualification-report --input target/benchmarks/qualification/pq1-cq2-gate-soak
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-gate-pr
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-gate-full
just bench::qualification-regression --input target/benchmarks/qualification/pq1-cq2-gate-soak
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, exact-preflight, and dependent PQ1 artifacts are identified above.

## Remaining Blocker

CQ2 still has 1,419 planned evidence owners. The next implementation blocker is `CQ-BIT-KERNELS`, where 384 of 388 owners remain planned across dense and sparse bit vectors, matrices, SIMD/scalar differential behavior, transposition, randomization, parsing, arithmetic, and boundary/resource contracts.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until all selected CQ2 domains are implemented.
