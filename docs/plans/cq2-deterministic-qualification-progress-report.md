# CQ2 Deterministic Qualification Progress Report

## Status

Implementation ownership is complete as of 2026-07-18. Clean evidence revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b` passed the PR tier's 145 selected parents and all 271 selected parents in both full and soak under correctness inventory `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`, with `local_modifications=false` throughout, and remains the latest complete historical all-domain checkpoint.

All eight selected deterministic domains are source-complete at exact upstream-symbol and exported-Rust-API granularity. Commit `763b04055f969668a4152e019ed62a3555cd0476` subsequently hardened the controller's expanded-owner admission check. Current offline replay therefore rejects the `6351fe0` request receipts as stale, exactly as the source-provenance contract requires. A fresh clean PR, full, and soak chain is required before all-domain execution can again be called source-current.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Latest complete all-domain correctness evidence revision: `6351fe0a529462efd9f18a88bd59c08cfec9f81b`, with historical PR, full, and soak reports under the current correctness digest but the pre-hardening controller source. The preceding focused revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` covers the three exact Pauli iterator prerequisites after milestone-audit and review strengthening through complete accepted-maximum traversal. Clean revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` covers the exact Pauli multiplication prerequisites before accepted Pauli timing, clean revision `f912cc3af1f13cc9fab798d69937c155d37d83a0` covers the exact transpose prerequisites before accepted transpose timing, clean revision `7b43b46d1c08f669264d009b8d3872ce86838f0e` covers the exact sparse-vector prerequisite before accepted sparse-XOR timing, clean revision `60b732c77f1828058fbd65ec6c5c4ad582467fd1` covers both exact bit-vector prerequisites before accepted `not_zero` timing, clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` previously covered the same prerequisites before historical dense-XOR timing, and clean revision `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f` separately covers the exact gate-name hash prerequisite. Those focused reports remain valid for their exact prerequisites but do not substitute for the pending source-current all-domain checkpoint.

Current correctness inventory digest: `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`. Core revision `8503f458eac09fe94dba54cf8d6e16e88e195df4` strengthens the exact Pauli iterator owner, clean evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` publishes its exact three-case prerequisite after literal sequence, complete yielded-plane, and accepted-maximum review fixes, and clean revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b` publishes the latest complete historical 271-parent family. The digest is unchanged, but controller source identity has advanced.

Current dependent performance inventory digest: `868eb831a034042b43573fed612af14db225421a2733bbf10e4a5eb2b515ec90`. It contains the same fifteen executable PQ2 product contracts and 21 exact threshold pairs as preceding digest `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`; only the synchronized CQ2 checklist evidence-status anchor changed. Clean evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` remains accepted historical iterator evidence under `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`, with 12 first-attempt passing reports and regressions, four replayed rollups, and two replayed completion receipts. Clean revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` remains historical accepted Pauli multiplication evidence, and earlier transpose and kernel evidence remains historical under its recorded inventories. No prior ratio is promoted under the refreshed inventory without a clean exact-group rerun.

The exact report at `target/qualification/pq2-m4-parse-print-full-ba70a52` selected all three shared parser and printer owners and passed all three. The report at `target/qualification/pq2-m4-gate-hash-full-c76b707` selected and passed `cq-evidence-qualification-bd20a013e903a05f`, which freezes the ordered 82-name table including `NOT_A_GATE` and every per-name Stim hash, and its exact dependent preflight passed before gate timing. The clean report at `target/qualification/pq2-m5-not-zero-full-60b732c` selected and passed `cq-evidence-qualification-b1530dc4e48e942d` and `cq-evidence-qualification-ba252d42660a41ce` before accepted `not_zero` timing; the earlier `target/qualification/pq2-m5-simd-bits-full-5d226c9` report selected the same owners before historical dense-XOR timing. The clean report at `target/qualification/pq2-m5-sparse-xor-final-7b43b46` selected and passed `cq-evidence-qualification-bea77c19e9ae0b24` before both sparse-XOR timing groups. All these clean reports bind preceding correctness digests and remain historical evidence.

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

- Thirty-seven implemented blocker-ledger parents, fourteen independently selected oracle-fixture parents, and nine focused qualification parents produce 60 implemented gate-contract evidence parents and zero planned owners.
- The exact partition assigns all 178 selected exported Rust API items. Of 540 relevant pinned-upstream records, 328 are ported through exact Rust ownership, 12 remain explicit semantic-mining references, and 200 are deferred with Python bindings or the public interactive graph and vector simulator products.
- Schema-version-2 `oracle/qualification-cases.json` mappings reuse canonical implemented or evidence-close blocker, oracle, and Rust-regression parents without duplicating terminal selectors. Explicit 64-bit, 128-bit, and 256-bit family declarations expand to exact upstream symbols and reject missing, duplicate, or unsupported word sizes.
- Focused qualification tests cover the 81-gate registry, the ordered 82-entry canonical-name and per-name Stim hash table including `NOT_A_GATE`, exact name lookup and rejection, unitary and decomposition metadata, gate-target text and values, target accessors, inversion, and target classification. Canonical gate semantic tests cover deterministic and noisy measurement-only and measure-reset gates, pair and product measurements, Pauli channels, depolarization, correlated and heralded errors, annotations, classical controls, SPP families, rejection contracts, and reset postconditions.
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

### Algebra

- Fifty-four exact Algebra parents and zero planned parents assign all 654 selected exported Rust API items. The 347 relevant pinned-upstream records are dispositioned as 12 ported Rust, 197 semantic-mining, and 138 deferred-product records; no selected Algebra record is hidden as not applicable or granted file-level credit.
- Dedicated exact parents cover Pauli values and iterators, flexible Paulis, single-qubit Cliffords, Clifford strings, Flow values and multiplication, Tableaux and Tableau iterators, commuting iterators, circuit conversion, inversion, random construction, stabilizer solving, and scoped unitary conversion. Existing exact fixture or Rust parents remain independently selected only when their complete selector proves the mapped contract.
- Review found that `Flow::multiply` had diverged from Stim's signed-input representation convention. Multiplication now preserves the left input sign and transfers only the relative input/output Pauli phase into the output sign, with exact signed-input and signed-output cases.
- `Flow::new` is fallible and shares one 65,536 aggregate measurement-plus-observable-term limit with text parsing. It stops on the first excess iterator item, bounds preallocation, and multiplication performs a bounded sorted symmetric-difference merge so terms may cancel before the resulting aggregate count is admitted.
- Pauli and Clifford values accept at most 1,048,576 qubits or entries; dense Tableau construction, circuit-to-Tableau conversion, and stabilizer solving accept at most 512 qubits; random-Tableau construction accepts at most 64 qubits; and unitary conversion accepts matrices through dimension 64. Annotation-only identity-flow generation separately admits at most 134,217,728 aggregate Pauli bits.
- `Circuit::to_tableau` handles Hermitian `SPP` and `SPP_DAG`, including repeated-qubit phase reduction, and rejects anti-Hermitian products. Compact repeat bodies are converted once and raised through identity-aware binary exponentiation instead of linear expansion, under a 16,777,216-unit aggregate width-squared composition budget.
- Eight exact resource parents prove maximum accepted and first rejected materialization, first-extra-item collection, cancellation before Flow admission, RNG non-consumption, dense-work allocation boundaries, logarithmic repeat semantics, the exact last accepted and first rejected repeat-work boundary, solver and iterator limits, unitary-limit precedence, and aggregate generated-flow output.
- Milestone audit and three GPT-5.6/max full-code-review lanes found and closed signed Flow multiplication, overclaimed exact parents, nested compact-repeat exhaustion, missing repeated-qubit `SPP` evidence, weak Clifford random and multiplication ownership, incomplete `inverse_skipping_signs` assertions, and missing maximum-accepted or pre-work allocation proofs. The final re-review reported no actionable findings.
- Resource rejection remains correctness evidence rather than a timed workload. Existing benchmark setup propagates constructor errors outside measured closures, and PQ2 must cite these exact passing prerequisites for accepted scales without timing rejection paths.

## Current CQ2 Inventory

| Feature | Implemented | Planned | Total |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 29 | 0 | 29 |
| `CQ-DEM-FORMAT` | 28 | 0 | 28 |
| `CQ-RESULT-FORMATS` | 39 | 0 | 39 |
| `CQ-GATE-CONTRACT` | 60 | 0 | 60 |
| `CQ-BIT-KERNELS` | 12 | 0 | 12 |
| `CQ-CIRCUIT-API` | 24 | 0 | 24 |
| `CQ-GENERATION` | 25 | 0 | 25 |
| `CQ-ALGEBRA` | 54 | 0 | 54 |
| **CQ2 total** | **271** | **0** | **271** |

These counts are independently selectable exact evidence parents, not raw upstream subcase or exported-API counts. A reviewed parent may own several upstream records or API items only when its one exact selector proves their complete common contract.

## Latest Complete Historical All-Domain CQ2 Evidence

All three reports bind correctness inventory `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`, Stim v1.16.0 commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, clean Stab revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b`, and `local_modifications=false`. They passed replay and exact preflight under that revision, but are historical after controller commit `763b04055f969668a4152e019ed62a3555cd0476`.
Request schema version 3, report and preflight schema version 6, and completion schema version 1 bind the exact selection, private executables, child environment, execution receipts, statistical completion, and controller-approved result set.

| Tier | Selected | Passed | Failed | Artifact | Request SHA-256 | Report SHA-256 | Completion SHA-256 | Preflight SHA-256 |
| --- | ---: | ---: | ---: | --- | --- | --- | --- | --- |
| PR | 145 | 145 | 0 | `target/qualification/cq2-deterministic-pr-6351fe0` | `d3bb1ec51e05df963b22d1f47cab245bfbd8e26be773e99db032cef269636435` | `bf272bc93ff2a753e56f074540f079f6e410ae3449cd0b06ac62b8bd5c2b124d` | `98398343440b4ea55c6cd5a1c162c7c0fb76624e62d54155c54e2e991c3fee02` | `9b702059547508ad8222aec334256b532467cb9ff3dfbb371e6dad28e042751f` |
| Full | 271 | 271 | 0 | `target/qualification/cq2-deterministic-full-6351fe0` | `f49835a28a3e0bd0d8486db8b52632db3ccddb74f280fbc89725e2d4ef9ca6b5` | `bcee2aeb5dfc2a492bc6e1b0286032e4c1e09f8732f4e95e115ead9b13de3b0d` | `f05c7bd829fa63fda3c93d6a8b25257ffc19f7eeae39d3cf309e5f510c6d78bb` | `1ba98e1901ffc05337e25d25305a12fb44395fd807ee5c8d9874b6b00b23bca2` |
| Soak | 271 | 271 | 0 | `target/qualification/cq2-deterministic-soak-6351fe0` | `5e8d747d085d9981fe9e746cabdc601442f303c5a9ebfc9bc46f89a7ae5dac92` | `00faa21d9d522012553746352ff9483030b37b0f5fe9e6ce5a295a5ec770cc4a` | `29566aee2ca0f44c2421bfb87c4135f5251248abd7fc7469c7ca318551ada3fd` | `c91762efdb264f6d92cff861d42b6f185bdbc9963aeb90613c83266c318927d3` |

Offline report regeneration passed for every tier.
Exact PR and soak preflight passed for `cq-evidence-blocker-083f1e2d245c4b57`, and exact full preflight passed for Algebra resource parent `cq-evidence-qualification-f7adaaac7766234d`, using the request and completion digests above.
Every full and soak parent completed with zero planned, deferred, skipped, or failed selections; PR selected its source-owned 145-case subset.

Each tier completed all 3,800,000 planned shots across 13 statistical owners and attempts, 32 independent comparisons, and 38 shot batches, with consumed exact false-positive bound `1.87388763315569654e-9` below declared suite budget `1.30000000000000026e-5`.
The full and soak totals intentionally match: all 13 selected statistical owners use exact legacy Cargo selectors that do not expose a seed override, and the source-owned plan requires soak to retain each case's one exact source seed instead of inventing an unexecutable panel.
The reports also retain 25 property corpus ids and eight exact resource cases.
This checkpoint therefore claims a fresh independently sealed soak execution, not multi-seed coverage for those legacy statistical owners.

## Refresh Audit And Review

The independent GPT-5.6/max milestone audit confirmed the exact 145-parent PR selection, both complete 271-parent selections, all outcomes, sealed identities, report replay, preflight, resource and property coverage, and statistical completion. It found one documentation overclaim about report-directory binding and one misuse of the controller's statistical multiplicity terms. The correctness plan now defines byte-identical artifact relocation beneath `target/qualification/` as intentional, `docs/plans/milestone-spec-gaps.md` records that resolved contract, and this report uses the exact 13 owners and attempts, 32 comparisons, and 38 batches. The follow-up audit reported no remaining milestone finding.

The independent GPT-5.6/max full code review found no P0 or P1 issue and confirmed that the artifacts were valid for their recorded source. It found the same statistical wording defect, stale current-evidence wording in the feature checklist and one historical PQ2 report, and a P3 fail-closed gap where new qualification cases could bypass the 2,048-owner cap through expanded word-size families. Commit `763b04055f969668a4152e019ed62a3555cd0476` counts the actual expanded owner set before admission, reuses that set during assignment, and adds a 2,049-owner regression. Replaying the older artifacts after that commit now fails closed with `run-request receipt metadata is stale`, so the reports remain historical instead of being relabeled as current. The review follow-up reported no remaining P0 through P3 implementation finding and no defect introduced by the fix.

Synchronizing the source-owned checklist row refreshed the performance inventory from `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf` to `868eb831a034042b43573fed612af14db225421a2733bbf10e4a5eb2b515ec90`. The generated diff changes only that checklist anchor and the top-level semantic digest; all fifteen executable product groups, 21 threshold pairs, workloads, scales, comparators, and correctness bindings are unchanged. Existing timing reports remain accepted historical evidence under their recorded inventories and are not promoted under the refreshed digest.

## Historical Clean All-Domain CQ2 Evidence

All three reports bind historical correctness inventory digest `deb6c025854e0e9dc555b45ee5afda33ac22b31c307d41d01731fa320a399f73`, Stim v1.16.0 commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, Stab revision `bae9e01cb3fedaf9d37958e6827b064c635b9898`, and `local_modifications=false`. They are not promotable under the refreshed digest.

| Tier | Selected | Passed | Failed | Artifact | Request digest | Report digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- |
| PR | 144 | 144 | 0 | `target/qualification/cq2-deterministic-pr-clean` | `16f8431163f42aac04db42595e811d99d23d36396f421a88e6b0481098536d53` | `9cefdbf4d5c4b5e0c5dcf8bc81fe270ad15332f494fecc57c2f8d5052662fb68` | `2e28276f9dc1519d3c3d362aca8d87d4b9b8d7373a3b9898ec23a8dacdbce06d` |
| Full | 270 | 270 | 0 | `target/qualification/cq2-deterministic-full-clean` | `0f1904404e516e7e47624a68fea588ac94946a7aa1b41c72fb3370076b23ddc7` | `0988aa43f876c43c47d1b24dcb57cc61e19455285336fe4712bd738a74215463` | `730229e0369486192bff0f4147810532ff226afe5235a27e97a151775776cb38` |
| Soak | 270 | 270 | 0 | `target/qualification/cq2-deterministic-soak-clean` | `64bb17d5a1946617eeb58c096bee4d8abd5f0a7025a9b099b74a561efd0068ef` | `ade3ccf60d9fe5d25ca93f5347e4f27bbfb121f9c71a218616f64c4798675e75` | `893b489e1301a980bc12c06248d77b063d4f71e344ca29b571cf93844366d868` |

Offline report regeneration passed for all three artifacts. PR, full, and soak each completed all 3,800,000 planned statistical shots with consumed false-positive bound `1.87388763315569654e-9`, below the declared suite budget `1.30000000000000026e-5`, and each report retained eight exact resource cases. Exact PR and soak preflight passed for `cq-evidence-blocker-083f1e2d245c4b57`; exact full preflight also passed for the Algebra Flow resource parent `cq-evidence-qualification-f7adaaac7766234d` using the controller-approved request and completion digests above.

## Historical Clean Generation-Refined Correctness Evidence

All three reports bind historical correctness inventory digest `d89a5f9eaba428fb72741c66ad74226820660e25e949123871c6c7ab86f82dd6`, Stim v1.16.0 commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, Stab revision `d0ecafd62794daad0ab5eb63d54c481a5e32a30b`, and `local_modifications=false`. They must not be promoted as evidence for the current gate-hash-refined digest.

| Tier | Selected | Passed | Failed | Artifact | Request digest | Report digest | Completion digest |
| --- | ---: | ---: | ---: | --- | --- | --- | --- |
| PR | 363 | 363 | 0 | `target/qualification/correctness/latest` | `f9affb08ccfab6c21e8c025d8d8938a806007f1c422f0f5a65d87f1b1e84c162` | `fe5ad8245305b6ec6ac0ce636be5dd1d4cf7ab2f613501b5647bec82110efaa1` | `8e66107cebe5552bc7298346adc2fc4f99196a2fdac1456274cd0501d2eff2d4` |
| Full | 541 | 541 | 0 | `target/qualification/correctness/full` | `190ede263f11820c8029a929b9fa6a9a45bdd0d5b5203fa338e76e700772169e` | `7feccbf9a54c6070f97ceab218dc547ecef764970ff7925dfb608e3044469e4d` | `8b21f2a6aead2a5b54b8050d23997b42e81313bdee3157974014583b54b3ab04` |
| Soak | 541 | 541 | 0 | `target/qualification/correctness/soak` | `3caa6f875ee991a3ef45074bb9dfd8a5490ce53afda0573f4a289dba7fcd6e1d` | `2b88a261b9ab96c175f6810e87f025ecbdded21c11c2408790b2361fb754b41b` | `dec639361b610278c24156b172381390162461a4781d19f822005fc7e2da4d13` |

Offline report regeneration passed for every tier. Exact PR, full, and soak preflight passed for the standalone CLI resource case `cq-evidence-qualification-31f61e175cdf6367` and Generation resource case `cq-evidence-qualification-a332fc484c66d6ba`. Every preflight reconstructed the controller-approved manifest, commit, selection, selector, output, request, completion, and result bindings. PR and full each completed all 4,218,400 planned statistical shots with consumed false-positive bound `2.67062845963454362e-6`; soak completed all 4,847,200 shots with consumed bound `5.98047030092843113e-6`, below the declared suite budget `3.20000000000000053e-5`.

## Historical Dependent PQ1 Refresh

The PQ1 group remains diagnostic infrastructure with `promotable=false`, `report-only` baseline eligibility, and zero checked product thresholds.

| Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Host | Artifact |
| --- | ---: | ---: | --- | ---: | --- | --- |
| PR | 3 | 1.014015 | [1.013541, 1.014200] | 0.000182 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-pr-schema13` |
| Full | 9 | 1.015160 | [1.013574, 1.016190] | 0.000801 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-full-schema13` |
| Soak | 15 | 1.015225 | [1.014722, 1.015822] | 0.000586 | Verified AArch64 | `target/benchmarks/qualification/pq1-generation-soak-schema13` |

All reports use schema version 13, the previous Generation-refined correctness and performance digests, `local_modifications=false` before and after execution, and commit `d0ecafd62794daad0ab5eb63d54c481a5e32a30b`. They are historical after the final Algebra ownership graph changed both frozen digests. Offline report validation passed, and regression replay returned `checked=0 report_only=true` for every tier. The PR, full, and soak report digests are `ff4d559937167dcf9c495838a22656de183fffdfe7b04fb7a2a74c9f43743a9c`, `eaeeeaf993521997c1aa2061a2c1fbeb4fceac8aba946291d9f5e1b46dd7db94`, and `9ddfd5154a0d768ae4e3e66308a13483fe46a2617257e2dc29f3d222dab480f5`. These ratios describe only the synthetic adapter protocol and must not be reported as Stab product performance.

## Verification

The latest complete historical 271-parent checkpoint used these commands from clean revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test cq2_algebra_resources --quiet
RUSTDOCFLAGS='-D warnings' cargo doc -p stab-core --no-deps
just oracle::run --milestone M6
just oracle::run --implemented-only
just qualification::correctness-regenerate --check
just qualification::correctness-check
just bench::qualification-regenerate --check
just bench::qualification-check
just qualification::correctness-run --tier pr --feature CQ-STIM-FORMAT --feature CQ-DEM-FORMAT --feature CQ-RESULT-FORMATS --feature CQ-GATE-CONTRACT --feature CQ-BIT-KERNELS --feature CQ-CIRCUIT-API --feature CQ-GENERATION --feature CQ-ALGEBRA --out target/qualification/cq2-deterministic-pr-6351fe0
just qualification::correctness-run --tier full --feature CQ-STIM-FORMAT --feature CQ-DEM-FORMAT --feature CQ-RESULT-FORMATS --feature CQ-GATE-CONTRACT --feature CQ-BIT-KERNELS --feature CQ-CIRCUIT-API --feature CQ-GENERATION --feature CQ-ALGEBRA --out target/qualification/cq2-deterministic-full-6351fe0
just qualification::correctness-run --tier soak --feature CQ-STIM-FORMAT --feature CQ-DEM-FORMAT --feature CQ-RESULT-FORMATS --feature CQ-GATE-CONTRACT --feature CQ-BIT-KERNELS --feature CQ-CIRCUIT-API --feature CQ-GENERATION --feature CQ-ALGEBRA --out target/qualification/cq2-deterministic-soak-6351fe0
just qualification::correctness-report --out target/qualification/cq2-deterministic-pr-6351fe0
just qualification::correctness-report --out target/qualification/cq2-deterministic-full-6351fe0
just qualification::correctness-report --out target/qualification/cq2-deterministic-soak-6351fe0
just qualification::correctness-preflight --out target/qualification/cq2-deterministic-pr-6351fe0 --case cq-evidence-blocker-083f1e2d245c4b57 --request-sha256 d3bb1ec51e05df963b22d1f47cab245bfbd8e26be773e99db032cef269636435 --completion-sha256 98398343440b4ea55c6cd5a1c162c7c0fb76624e62d54155c54e2e991c3fee02
just qualification::correctness-preflight --out target/qualification/cq2-deterministic-full-6351fe0 --case cq-evidence-qualification-f7adaaac7766234d --request-sha256 f49835a28a3e0bd0d8486db8b52632db3ccddb74f280fbc89725e2d4ef9ca6b5 --completion-sha256 f05c7bd829fa63fda3c93d6a8b25257ffc19f7eeae39d3cf309e5f510c6d78bb
just qualification::correctness-preflight --out target/qualification/cq2-deterministic-soak-6351fe0 --case cq-evidence-blocker-083f1e2d245c4b57 --request-sha256 5e8d747d085d9981fe9e746cabdc601442f303c5a9ebfc9bc46f89a7ae5dac92 --completion-sha256 29566aee2ca0f44c2421bfb87c4135f5251248abd7fc7469c7ca318551ada3fd
just maintenance::pre-commit
```

The clean PR, full, soak, offline-report, and exact-preflight artifacts are authoritative historical correctness evidence for their recorded source. They do not satisfy the current controller-source preflight. The historical dependent PQ1 artifacts remain harness evidence only and do not qualify CQ2 product performance.

## Next Milestone

CQ2 has no remaining planned owner or implementation blocker. Its current evidence blocker is one clean PR, full, and soak rerun plus offline replay and exact preflight from the same post-hardening revision. Complete that chain before the selected Clifford-string PQ2 slice consumes any CQ2 preflight or publishes promotable timing evidence.
