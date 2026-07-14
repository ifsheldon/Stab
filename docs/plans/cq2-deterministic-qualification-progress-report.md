# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The selected `.stim`, `.dem`, result-format, gate-contract, and bit-kernel domains are complete at exact upstream-symbol and exported-Rust-API granularity. This checkpoint does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Latest clean global evidence revision: `d4301cc1085680ff650f9e0474e075998c14c4bd` with `local_modifications=false`, for the current bit-refined digest.

Current correctness inventory digest after bit-kernel reconciliation: `2b2f0456e568b86a973d4b9077b9688ab9f7748af1bd9cd349e2a2abf217d638`.

Current dependent performance inventory digest: `4e31a348b0c796ae4c4400369c70019eff8fa991592f201c80e7fee7d8718f7a`.

Clean current-digest CQ1 and dependent PQ1 reports are published at the artifact paths below.

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

### Bit Kernels

- Eight focused qualification parents plus four independently selected exact M5 oracle-fixture parents produce 12 implemented bit-kernel evidence parents and zero planned owners.
- The exact partition assigns all 274 selected exported Rust API items and all 82 selected upstream semantic records: five `bit_ref`, nine bit-table, 24 owned-bit-vector, 18 range-view, seven transpose-helper, 12 SIMD-word, four sparse-vector, and three integer-twiddle records.
- Deterministic tests span zero width, unaligned tails, 64-bit word boundaries, 256-bit portable-SIMD lane boundaries, multi-lane widths through 65,537 bits, dirty padding, checked range overflow, clone independence, matrix self-overlap, rectangular transpose rejection, sparse stack-to-heap transitions, and dense-versus-sparse symmetric-difference agreement.
- Domain audit found that self-masked matrix row XOR copied the complete row before applying `value &= !mask`, contradicting the constant-scratch resource contract. The path now uses an in-place portable-SIMD AND-NOT kernel, and direct allocation-counter checks prove that repeated preallocated vector and matrix mutation performs zero allocations.
- The 168 pinned C++ cases for move state, mutable aliasing and range views, destructive or preserving resize, padded lane layout, arithmetic, shifts, raw random fill, table parsing, gather, concatenation or quadrants, triangular inverse, table-only sparse storage, and unexposed predicates are explicitly `not-applicable`; Stab exposes no corresponding selected Rust bit contract, and safe Rust excludes the C++ mutable-aliasing shape by construction.
- Raw random generation remains owned by typed `CQ-ALGEBRA` APIs that accept caller-owned `rand::Rng`; no raw bit-vector random-fill API was added merely to mirror an internal C++ helper. Four broad imported M5 fixtures are retained as supporting provenance on the new exact parents, while four already-exact M5 fixtures remain independent evidence.

The dependent PQ0 inventory was regenerated because the correctness digest, owner ids, and selected bit-kernel dispositions changed. No performance disposition, threshold, waiver, comparator classification, or runtime group was relaxed.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 32 | 0 | 32 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 59 | 0 | 59 |
| `CQ-BIT-KERNELS` | 12 | 0 | 12 |
| `CQ-CIRCUIT-API` | 37 | 325 | 362 |
| `CQ-GENERATION` | 15 | 75 | 90 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **223** | **1,035** | **1,258** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 338 | 338 | 0 | `target/qualification/correctness/latest` | `8398a56feced99d3fead593932aeb34684b4596cbbb3e3b4133a7c2eb7a2c6b3` | `0adbdaaae35b7c32ccc4477a4e088c2eb6045543a50eb558c1ac11c9b76d7416` |
| Full | 509 | 509 | 0 | `target/qualification/correctness/full` | `8d874cee498dfa02329d4da6841422d673d3045e5f70083d87caeb37f76837e0` | `100e5f63123037094538af05ad5da8b1d631dc467700b4fd7b6a29c266332b74` |
| Soak | 509 | 509 | 0 | `target/qualification/correctness/soak` | `b34aac11d03a19b064bd2d031f06ee948b71d249b3c12425bf61b8599ddf06cc` | `39b9342975ca078795342aef6acde479ff92470cd81d95e6d9e706d4f53c470d` |

Offline report regeneration passed for every tier. Exact PR preflight passed for `cq-evidence-qualification-1e1f30bac217eee5`, the bit-vector copy, range, and tail parent. Exact full and soak preflight passed for `cq-evidence-qualification-b1530dc4e48e942d`, the bit-vector logical-operation parent. Every preflight reconstructed the controller-approved manifest, commit, selection, selector, output, request, completion, and result bindings. PR and full each completed all 4,218,400 planned statistical shots with consumed false-positive bound `2.67062845963454362e-6`; soak completed all 4,847,200 shots with consumed bound `5.98047030092843113e-6`, below the declared suite budget `3.20000000000000053e-5`.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Host | Artifact |
| --- | ---: | ---: | --- | ---: | --- | --- |
| PR | 3 | 1.016046 | [1.015463, 1.016840] | 0.000575 | Verified | `target/benchmarks/qualification/pq1-bit-pr-schema13` |
| Full | 9 | 1.014565 | [1.014177, 1.015580] | 0.000383 | Verified | `target/benchmarks/qualification/pq1-bit-full-schema13` |
| Soak | 15 | 1.015206 | [1.014642, 1.016074] | 0.000576 | Verified | `target/benchmarks/qualification/pq1-bit-soak-schema13` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `d4301cc1085680ff650f9e0474e075998c14c4bd`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. These ratios describe only the synthetic adapter protocol and must not be reported as Stab product performance.

## Verification

Passing checks for the current checkpoint:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_gate_contract --quiet
cargo test -p stab-core --test cq2_bit_kernels --quiet
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
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full --out target/qualification/correctness/full
just qualification::correctness-run --tier soak --out target/qualification/correctness/soak
just qualification::correctness-report --out target/qualification/correctness/latest
just qualification::correctness-report --out target/qualification/correctness/full
just qualification::correctness-report --out target/qualification/correctness/soak
just qualification::correctness-preflight --out target/qualification/correctness/latest --case cq-evidence-qualification-1e1f30bac217eee5 --request-sha256 8398a56feced99d3fead593932aeb34684b4596cbbb3e3b4133a7c2eb7a2c6b3 --completion-sha256 0adbdaaae35b7c32ccc4477a4e088c2eb6045543a50eb558c1ac11c9b76d7416
just qualification::correctness-preflight --out target/qualification/correctness/full --case cq-evidence-qualification-b1530dc4e48e942d --request-sha256 8d874cee498dfa02329d4da6841422d673d3045e5f70083d87caeb37f76837e0 --completion-sha256 100e5f63123037094538af05ad5da8b1d631dc467700b4fd7b6a29c266332b74
just qualification::correctness-preflight --out target/qualification/correctness/soak --case cq-evidence-qualification-b1530dc4e48e942d --request-sha256 b34aac11d03a19b064bd2d031f06ee948b71d249b3c12425bf61b8599ddf06cc --completion-sha256 39b9342975ca078795342aef6acde479ff92470cd81d95e6d9e706d4f53c470d
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-bit-pr-schema13
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-bit-full-schema13
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-bit-soak-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-bit-pr-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-bit-full-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-bit-soak-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-bit-pr-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-bit-full-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-bit-soak-schema13
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, exact-preflight, and dependent PQ1 artifacts above are authoritative for the current bit-refined digests.

## Remaining Blocker

CQ2 still has 1,035 planned evidence owners. The active implementation blocker is `CQ-CIRCUIT-API`, where 325 of 362 owners remain planned across construction, mutation, introspection, coordinates, repeat handling, and selected transforms.

No external dependency or user decision blocks this work. CQ2 milestone audit and GPT-5.6/max full-code-review remain pending until all selected CQ2 domains are implemented.
