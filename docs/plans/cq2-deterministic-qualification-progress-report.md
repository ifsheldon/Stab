# CQ2 Deterministic Qualification Progress Report

## Status

In progress as of 2026-07-14.

The selected `.stim`, `.dem`, result-format, gate-contract, bit-kernel, and circuit-API domains are complete at exact upstream-symbol and exported-Rust-API granularity. This checkpoint does not complete CQ2.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Latest clean global evidence revision: `1eaf3740c5819271be30a37ce4b6f69e2010a2ba` with `local_modifications=false`, for the current circuit-refined digest.

Current correctness inventory digest after circuit-API reconciliation: `f30a56853dbc9334d1ba91a400da3a40c108d4ee2aa748d4edc9a5662093ba11`.

Current dependent performance inventory digest: `4ebbf8bc85ebeb722fcf3e9eefa4796b57ba61b2773fa64eef37e70dca41a744`.

Clean current-digest CQ1 reports are published at the artifact paths below. The dependent PQ1 refresh awaits the clean documentation-and-inventory commit because the checklist clarification changed the performance digest without changing correctness.

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

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 29 | 0 | 29 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 59 | 0 | 59 |
| `CQ-BIT-KERNELS` | 12 | 0 | 12 |
| `CQ-CIRCUIT-API` | 24 | 0 | 24 |
| `CQ-GENERATION` | 12 | 75 | 87 |
| `CQ-ALGEBRA` | 1 | 635 | 636 |
| **CQ2 total** | **204** | **710** | **914** |

These counts are evidence owners, not an estimate of required new test functions. Reviewed exact parents may close several owners only when one selector proves their complete common contract.

## Clean Correctness Evidence

| Tier | Selected | Passed | Failed | Artifact | Request digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- |
| PR | 354 | 354 | 0 | `target/qualification/correctness/latest` | `33c6cdb706124de39c959583af18d9f83cb57b0199ed5437ab756f35a39ea0bd` | `a85eaf987a356f309252fc4d0286ea8424954a6026f72835181741b035026d1f` |
| Full | 527 | 527 | 0 | `target/qualification/correctness/full` | `5b59782b51b506bad886e5793de02d9c57eefa907af2b9d7c9b64b13aa5eb186` | `939dcaa30e2b6a1663345a039b069a57f4202cd356b4b0e14f1ffb9b92d42c59` |
| Soak | 527 | 527 | 0 | `target/qualification/correctness/soak` | `b677425b10cba61bd0d49aa387284494a5a015e18bda8d8ad32d68304e3d7f20` | `d113dde05b51a1f055b101e15fbeea6f1d03aad7820b51d0a09df733d6c6233b` |

Offline report regeneration passed for every tier. Exact PR, full, and soak preflight passed for `cq-evidence-qualification-0bd1d8d0ab37a23d`, the Circuit repetition and checked-overflow parent. Every preflight reconstructed the controller-approved manifest, commit, selection, selector, output, request, completion, and result bindings. PR and full each completed all 4,218,400 planned statistical shots with consumed false-positive bound `2.67062845963454362e-6`; soak completed all 4,847,200 shots with consumed bound `5.98047030092843113e-6`, below the declared suite budget `3.20000000000000053e-5`.

## Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds. The earlier `3c49ecdfde72eca0a331b9949f2bee4dd6f300ebfa015d779b4f5cbb7ac91355` reports validated successfully but became stale when the checklist evidence note changed the dependent digest. Run PR, full, and soak from the clean commit containing `4ebbf8bc85ebeb722fcf3e9eefa4796b57ba61b2773fa64eef37e70dca41a744`, then publish their exact ratios and artifact paths in a follow-up evidence commit.

## Verification

Passing checks for the current checkpoint:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_circuit_api --test circuit_api --test circuit_transforms --test circuit_simplify --test mbqc_decomposition --quiet
cargo test -p stab-oracle qualification::classification::tests --quiet
cargo test -p stab-oracle qualification::inventory::tests:: --quiet
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
just qualification::correctness-preflight --out target/qualification/correctness/latest --case cq-evidence-qualification-0bd1d8d0ab37a23d --request-sha256 33c6cdb706124de39c959583af18d9f83cb57b0199ed5437ab756f35a39ea0bd --completion-sha256 a85eaf987a356f309252fc4d0286ea8424954a6026f72835181741b035026d1f
just qualification::correctness-preflight --out target/qualification/correctness/full --case cq-evidence-qualification-0bd1d8d0ab37a23d --request-sha256 5b59782b51b506bad886e5793de02d9c57eefa907af2b9d7c9b64b13aa5eb186 --completion-sha256 939dcaa30e2b6a1663345a039b069a57f4202cd356b4b0e14f1ffb9b92d42c59
just qualification::correctness-preflight --out target/qualification/correctness/soak --case cq-evidence-qualification-0bd1d8d0ab37a23d --request-sha256 b677425b10cba61bd0d49aa387284494a5a015e18bda8d8ad32d68304e3d7f20 --completion-sha256 d113dde05b51a1f055b101e15fbeea6f1d03aad7820b51d0a09df733d6c6233b
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, and exact-preflight artifacts above are authoritative for the current correctness digest. Current-digest dependent PQ1 artifacts remain pending until the clean documentation-and-inventory commit exists.

## Remaining Blocker

CQ2 still has 710 planned evidence owners. The active implementation blocker is `CQ-GENERATION`, where 75 of 87 owners remain planned; `CQ-ALGEBRA` then has 635 of 636 owners planned.

No external dependency or user decision blocks this work. The Circuit API slice passed milestone audit and GPT-5.6/max full-code-review with every confirmed finding closed; the final CQ2-wide audit and review remain pending until Generation and Algebra are implemented.
