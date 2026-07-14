# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-15.

The selected `.stim`, `.dem`, result-format, gate-contract, bit-kernel, circuit-API, and Generation domains are source-complete at exact upstream-symbol and exported-Rust-API granularity. Generation's milestone audit, GPT-5.6/max full-code-review, broad verification, clean current-digest correctness execution, exact preflight, and dependent PQ1 publication are complete with every confirmed finding fixed. This checkpoint does not complete CQ2 because `CQ-ALGEBRA` remains open.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Latest clean correctness evidence revision: `d0ecafd62794daad0ab5eb63d54c481a5e32a30b` with `local_modifications=false`, for the historical Generation-refined digest.

Current correctness inventory digest after Algebra resource-admission regeneration: `7e42ddddd662593b56f0bd67885b74babf9a96319de990e4f2cb6218638edea5`.

Current dependent performance inventory digest: `67bcbfcf2d991c883b6d889bf48b4d9b8c09bcb52bdbd6dc1e041b6162a30193`.

Clean current-digest PR, full, and soak correctness reports are not yet published. The reports from the reviewed committed revision above are historical because the source-owned Algebra dispositions changed both frozen digests.

## Delivered Slices

### `.stim` Format

- Twenty-four qualification parents and five independently selected exact oracle-fixture parents produce 29 implemented `.stim` evidence parents and zero planned owners.
- Four model-level PF1 fixtures for concatenation, detector coordinates, item insertion and removal, and iteration now belong to `CQ-CIRCUIT-API`; the odd-CX target rejection belongs to `.stim` validation instead of being deferred merely because its pinned test is Python.
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

### Circuit API

- Twenty focused qualification parents plus four independently selected exact PF1 oracle-fixture parents produce 24 implemented circuit-API evidence parents and zero planned owners.
- The exact partition assigns all 185 selected exported Rust API items. Of 109 relevant pinned-upstream records, nine have direct ported-Rust ownership, 40 are semantic-mining inputs bound to focused Rust parents, and 60 Python object-shape, operator, indexing, coercion, copying, hashing, pickle, file-like, and diagram cases remain explicitly deferred with their products.
- Focused tests cover instruction constructors and accessors, the complete pinned target-group matrix including empty `MPP`, segmented-target preservation, typed identifiers, all `CircuitError` variants, `CircuitItem` and repeat-block value semantics, append and fusion rules, tag stripping, iterator clone independence, aggregate counts, checked overflow, coordinates, clearing, repetition, flattening, noise removal, decomposition, simplification, and MBQC decomposition across every implemented gate.
- Pinned C++ aggregate count tests that combine portable non-overflow behavior with `UINT64_MAX` saturation are `not-applicable` as complete symbols because Stab's selected Rust API returns checked `Result` errors. Focused Rust and Python-derived parents own the shared portable semantics without claiming saturation parity.
- Milestone audit moved inverse behavior and flow utilities to `CQ-FLOW-UTILS`, sampling and determined-measurement behavior to `CQ-SAMPLING`, and generation or search behavior to their own domains. Three GPT-5.6/max full-code-review passes found and closed incomplete identity-error, repeat-block, exhaustive-error, selector-boundary, and empty-`MPP` evidence; no confirmed Circuit API finding remains.

The dependent PQ0 inventory was regenerated because circuit owner ids, upstream dispositions, and acceptance state changed. No performance disposition, threshold, waiver, comparator classification, or runtime group was relaxed.

### Generation

- Thirteen focused qualification parents plus twelve retained exact fixture and prior-owner parents produce 25 implemented Generation evidence parents and zero planned owners.
- The ownership partition assigns all 107 selected exported Rust API items and all 15 relevant pinned-upstream records. Fourteen portable records have direct focused ownership or semantic-mining bindings; `circuit_pybind_test.py::test_circuit_generation_errors` remains one explicit `deferred-product` symbol because its complete contract enters through Python `Circuit.generated` string dispatch. Generation independently owns its typed distance, round, task, and family constraints; `Probability` value validation remains in its own qualification domain and is not claimed by this slice.
- Focused exact tests cover `CodeDistance`, `RoundCount`, every task enum and generation parameter object, immutable noise builders, generated-circuit value semantics and accessors, complete noisy repetition, rotated and unrotated surface, and color-code circuits and layouts, larger pinned layouts, invalid color-family combinations, and all six selected helper semantics.
- The complete upstream no-noise family, task, distance, and round matrix checks exact detector-count formulas, one observable, absence of noise instructions, and deterministic conversion of every circuit into an error-free detector error model. Representative sampled execution covers every task, a portable 256-shot multiword batch, and the pinned d5/r10 five-shot 240-detector shape without relying on probabilistic per-cell sampling to prove a deterministic invariant.
- Materializing generators preflight an exact projected physical-qubit count against 131,072 before coordinate maps, instruction vectors, layout text, or CLI output are allocated. Repetition distance 2047, rotated surface distance 256, unrotated surface distance 181, and valid color distance 341 are accepted; the first valid rejected boundaries project 132,097, 131,769, and 132,355 qubits respectively. CLI evidence proves rejection happens before `--out` is created, and maximum round counts remain folded.
- Pinned Stim's nominal generator distance domain extends through 2047, so the surface and color cap is documented as deliberate bounded-materialization behavior rather than exact acceptance parity. No performance threshold, waiver, comparator classification, or runtime group was changed while regenerating the dependent inventory.
- Milestone audit found that typed and family rejection tests checked only generic errors or message fragments and that six private helper parents overclaimed constant-scratch resource behavior. The corrected tests match `CircuitError::InvalidDomainValue` plus exact `kind` and `value` fields, while the private fixed-target helper parents make no independent resource claim and point to the public generator admission parent.
- Review also found that one broad generator-helper fixture could not remain an atomic primary. It is split into six exact independently runnable fixture rows, and qualification validation permits an imported exact fixture to become supporting provenance only when its normalized exact Cargo selector is identical to the reviewed qualification primary. Duplicate terminal primaries remain rejected.
- Three earlier GPT-5.6/max full-code-review passes and the final focused GPT-5.6/max inventory review found incomplete deterministic matrix evidence, weak pre-allocation proof, stale fixture selectors, incorrect command documentation, Python probability overclaim, missing CLI resource ownership, stale dependent digests, stale CQ0/CQ1 current-evidence wording, and one owner-count drift. Every confirmed finding is fixed, and clean committed-head execution closed the final Generation acceptance requirement.

### Algebra Resource Admission

- CQ2 ownership reconciliation revealed that several public caller-size-driven Algebra constructors were infallible despite allocating dense state or initiating expensive work. The resolved contract exposes typed limits through `StabilizerResource` and makes Pauli, flexible-Pauli, Clifford, Tableau, random-Tableau, and reusable Pauli-iterator construction fallible.
- Pauli and Clifford values accept at most 1,048,576 qubits or entries; dense Tableau construction, circuit-to-Tableau conversion, and stabilizer solving accept at most 512 qubits; random-Tableau construction accepts at most 64 qubits; and unitary conversion accepts matrices through dimension 64. Clifford concatenation and repetition check projected size, repetition overflow has a resource-specific error, and empty repetition is constant-work.
- Annotation-only identity-flow generation uses a separate 134,217,728 aggregate Pauli-bit budget. This preserves the established 2,049-qubit high-index annotation regression while rejecting larger quadratic output before constructing unchecked internal Pauli rows.
- `crates/stab-core/tests/cq2_algebra_resources.rs` proves representative accepted boundaries, first rejection, first-extra-item collection, RNG non-consumption, circuit-derived rejection, solver and iterator limits, unitary-limit precedence, aggregate flow-output admission, and constant-work empty repetition. The complete `stab-core` suite and targeted `stab-core` plus `stab-bench` Clippy checks pass.
- The focused milestone audit found that flow checking and time-reversal validation still selected their former 8,192-qubit dense path above the new 512-qubit Tableau cap, that iterable-constructor documentation overclaimed allocation-free rejection, and that one public Rustdoc link was broken. Dispatch now falls back to sparse validation at 513 qubits with a direct regression, documentation distinguishes explicit-size and unknown-length iterable admission, and Rustdoc passes with warnings denied.
- Resource rejection is correctness evidence rather than a timed workload. Existing M6 benchmark setup now propagates constructor errors outside measured closures, and the performance plan requires accepted benchmark scales to cite the exact resource prerequisites without timing rejection paths.
- This closes the deterministic-materialization spec gap but does not close the Algebra qualification slice. Exact semantic parents, public-API assignments, regenerated CQ0 and PQ0 inventories, clean evidence, milestone audit, and GPT-5.6/max review remain required.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 29 | 0 | 29 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 59 | 0 | 59 |
| `CQ-BIT-KERNELS` | 12 | 0 | 12 |
| `CQ-CIRCUIT-API` | 24 | 0 | 24 |
| `CQ-GENERATION` | 25 | 0 | 25 |
| `CQ-ALGEBRA` | 1 | 457 | 458 |
| **CQ2 total** | **217** | **457** | **674** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Historical Clean Generation-Refined Correctness Evidence

All three reports bind historical correctness inventory digest `d89a5f9eaba428fb72741c66ad74226820660e25e949123871c6c7ab86f82dd6`, Stim v1.16.0 commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, Stab revision `d0ecafd62794daad0ab5eb63d54c481a5e32a30b`, and `local_modifications=false`. They must not be promoted as evidence for the current Algebra-refined digest.

| Tier | Selected | Passed | Failed | Artifact | Request digest | Report digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- |
| PR | 363 | 363 | 0 | `target/qualification/correctness/latest` | `f9affb08ccfab6c21e8c025d8d8938a806007f1c422f0f5a65d87f1b1e84c162` | `fe5ad8245305b6ec6ac0ce636be5dd1d4cf7ab2f613501b5647bec82110efaa1` | `8e66107cebe5552bc7298346adc2fc4f99196a2fdac1456274cd0501d2eff2d4` |
| Full | 541 | 541 | 0 | `target/qualification/correctness/full` | `190ede263f11820c8029a929b9fa6a9a45bdd0d5b5203fa338e76e700772169e` | `7feccbf9a54c6070f97ceab218dc547ecef764970ff7925dfb608e3044469e4d` | `8b21f2a6aead2a5b54b8050d23997b42e81313bdee3157974014583b54b3ab04` |
| Soak | 541 | 541 | 0 | `target/qualification/correctness/soak` | `3caa6f875ee991a3ef45074bb9dfd8a5490ce53afda0573f4a289dba7fcd6e1d` | `2b88a261b9ab96c175f6810e87f025ecbdded21c11c2408790b2361fb754b41b` | `dec639361b610278c24156b172381390162461a4781d19f822005fc7e2da4d13` |

Offline report regeneration passed for every tier. Exact PR, full, and soak preflight passed for the standalone CLI resource case `cq-evidence-qualification-31f61e175cdf6367` and Generation resource case `cq-evidence-qualification-a332fc484c66d6ba`. Every preflight reconstructed the controller-approved manifest, commit, selection, selector, output, request, completion, and result bindings. PR and full each completed all 4,218,400 planned statistical shots with consumed false-positive bound `2.67062845963454362e-6`; soak completed all 4,847,200 shots with consumed bound `5.98047030092843113e-6`, below the declared suite budget `3.20000000000000053e-5`.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Host | Artifact |
| --- | ---: | ---: | --- | ---: | --- | --- |
| PR | 3 | 1.014015 | [1.013541, 1.014200] | 0.000182 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-pr-schema13` |
| Full | 9 | 1.015160 | [1.013574, 1.016190] | 0.000801 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-full-schema13` |
| Soak | 15 | 1.015225 | [1.014722, 1.015822] | 0.000586 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-soak-schema13` |

All reports use schema version 13, the current correctness and performance digests, `local_modifications=false` before and after execution, and commit `d0ecafd62794daad0ab5eb63d54c481a5e32a30b`. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. The PR, full, and soak report digests are `ff4d559937167dcf9c495838a22656de183fffdfe7b04fb7a2a74c9f43743a9c`, `eaeeeaf993521997c1aa2061a2c1fbeb4fceac8aba946291d9f5e1b46dd7db94`, and `9ddfd5154a0d768ae4e3e66308a13483fe46a2617257e2dc29f3d222dab480f5`. These ratios describe only the synthetic adapter protocol and must not be reported as Stab product performance.

## Verification

Passing checks for the current checkpoint:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_circuit_api --test circuit_api --test circuit_transforms --test circuit_simplify --test mbqc_decomposition --quiet
cargo test -p stab-core --test circuit_generation --quiet
cargo test -p stab-core --test cq2_algebra_resources --quiet
cargo test -p stab-core --lib circuit_generation::tests:: --quiet
cargo test -p stab-cli --lib tests::generation::gen_rejects_quadratic_outputs_before_writing --quiet -- --exact
cargo test -p stab-oracle qualification::classification::tests --quiet
cargo test -p stab-oracle qualification::inventory::tests:: --quiet
cargo test -p stab-oracle qualification::inventory::qualification_cases::tests:: --quiet
just oracle::run --milestone M7
just oracle::run --implemented-only
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
just qualification::correctness-preflight --out target/qualification/correctness/latest --case cq-evidence-qualification-a332fc484c66d6ba --request-sha256 f9affb08ccfab6c21e8c025d8d8938a806007f1c422f0f5a65d87f1b1e84c162 --completion-sha256 8e66107cebe5552bc7298346adc2fc4f99196a2fdac1456274cd0501d2eff2d4
just qualification::correctness-preflight --out target/qualification/correctness/full --case cq-evidence-qualification-a332fc484c66d6ba --request-sha256 190ede263f11820c8029a929b9fa6a9a45bdd0d5b5203fa338e76e700772169e --completion-sha256 8b21f2a6aead2a5b54b8050d23997b42e81313bdee3157974014583b54b3ab04
just qualification::correctness-preflight --out target/qualification/correctness/soak --case cq-evidence-qualification-a332fc484c66d6ba --request-sha256 3caa6f875ee991a3ef45074bb9dfd8a5490ce53afda0573f4a289dba7fcd6e1d --completion-sha256 dec639361b610278c24156b172381390162461a4781d19f822005fc7e2da4d13
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-generation-pr-schema13
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-generation-full-schema13
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-generation-soak-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-generation-pr-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-generation-full-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-generation-soak-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-generation-pr-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-generation-full-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-generation-soak-schema13
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, exact-preflight, and dependent PQ1 artifacts above are authoritative for the Generation-refined digests recorded in this report.

## Remaining Blocker

CQ2 still has 457 planned evidence owners, all in `CQ-ALGEBRA`. Generation source ownership and acceptance remain closed at 25 of 25; its clean PR, full, and soak correctness evidence plus dependent report-only PQ1 publication are historical after the Algebra digest change.

No external dependency or user decision blocks this work. The active implementation blocker is `CQ-ALGEBRA`, where 457 of 458 owners remain planned; final CQ2-wide audit and review remain pending until Algebra is implemented.
