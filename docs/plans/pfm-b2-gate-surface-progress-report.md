# PFM-B2 Gate-Surface Semantic Progress Report

## Status

PFM-B2 implementation and executable oracle evidence are complete. The initial semantic and oracle work landed in `f60ea17` and `e7a67a0`; review remediation landed in `f1f6e42`, oracle hardening in `6bdff8b`, and split benchmark evidence in `6576273`.
The canonical table classifies all 81 gates, 19 semantic families, 22 accepted target patterns, and eight surfaces without an unknown state.
The 18 original `pfm3-contract-*` records remain the semantic-family rollups, but implementation exposed that 11 of them aggregated 30 exact pinned-Stim test anchors and therefore could not honestly serve as completion evidence.
Those aggregations are now split into 37 implemented ledger cases with 37 independent exact selectors and oracle shards; the full source ledger has 165 cases and no planned row.
The clean timing and allocation reports from `6576273` are superseded by final-review remediation: `2f46c33` removes the sweep-reference record copy, `25f352b` unifies canonical surface and statistical boundaries, `8ab85e4` requires exact upstream gate markers, and `fb47b03` separates ordinary detection from forced detector-frame timing while suppressing heterogeneous report-only medians.
Fresh clean timing and allocation reports plus a follow-up GPT-5.6/max re-review are the remaining process gates.

## Selected Surfaces

Each case owns the same complete surface set:

1. Parser acceptance and target-pattern classification.
2. Measurement-sampler compilation and execution.
3. Deterministic reference sampling.
4. Detection-converter compilation and record conversion.
5. Direct detector-frame execution selected by a Pauli-backed observable.
6. Detection sampling through the ordinary or frame-backed path as applicable.
7. Circuit-to-DEM error analysis.
8. Circuit flow generation.

Tests must invoke the production implementation for every applicable surface.
Contract-table assertions alone do not count as execution evidence.
`semantic_noop`, `annotation`, and `not_applicable` decisions require explicit unchanged-state or structural assertions instead of merely accepting the input.

## Executable Case Matrix

| Ledger case | Families and shapes | Required semantic evidence | Comparator |
| --- | --- | --- | --- |
| `pfm3-contract-fixed-tableau` | Every canonical `fixed-tableau` gate, empty and legal plain-qubit groups | Gate followed by its inverse matches the identity across reference, sampling, conversion, ordinary detection, frame detection, analysis, and flow generation | State equivalence |
| `pfm3-contract-measure-reset` | `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`, and `RY`, including inverted measurement targets and empty groups | Exact deterministic records and reset postconditions plus independent 100,000-shot `p=0.05` MRX, MRY, and MR flip-rate shards | Exact and statistical |
| `pfm3-contract-pair-measurement` | `MXX`, `MYY`, and `MZZ`, including inverted targets and empty groups | Exact Bell or basis-state parity records and cross-surface consistency | Exact |
| `pfm3-contract-mpp-deterministic` | Hermitian single-term, multi-term, repeated-qubit, inverted, empty, and pinned four-body MPP groups | Pinned four-body parity invariants, exact conversion and detector silence for deterministic reference records, analyzer silence, and nonempty flow generation | Semantic invariant |
| `pfm3-contract-mpp-anti-hermitian-rejection` | Anti-Hermitian MPP products accepted by the parser | Every semantic surface rejects with the typed anti-Hermitian error class and no surface silently lowers or skips the product | Error class |
| `pfm3-contract-mpad-deterministic` | Constant zero, constant one, duplicate constants, and empty MPAD groups | Exact records, conversion, detector events, analyzer output, and flows | Exact |
| `pfm3-contract-mpp-stochastic` | `MPP(0.25)` | 100,000 seeded shots with `mpp-zero=0.75` and `mpp-one=0.25`, plus converter and frame consistency | Statistical |
| `pfm3-contract-mpad-stochastic` | `MPAD(0.25)` | 100,000 seeded shots with `mpad-zero=0.75` and `mpad-one=0.25`, plus converter and frame consistency | Statistical |
| `pfm3-contract-spp` | `SPP`, `SPP_DAG`, Hermitian products, inversions, empty groups, and anti-Hermitian rejection | Exact state equivalence against decomposition on all semantic surfaces and the same typed rejection policy as MPP | State equivalence |
| `pfm3-contract-pauli-noise` | `X_ERROR`, `Y_ERROR`, and `Z_ERROR`, including empty groups | Bell-state classification after independent probability-one-half gates gives `identity=x=y=z=0.25`; reference, converter, and flow paths remain unchanged | Statistical |
| `pfm3-contract-pauli-channels` | `PAULI_CHANNEL_1` and `PAULI_CHANNEL_2`, including empty groups | One-qubit channel rates `I=0.4,X=0.1,Y=0.2,Z=0.3`; two-qubit channel rates `II=0.4` and every nonidentity Pauli pair `0.04` | Statistical |
| `pfm3-contract-identity-noise` | `I_ERROR` and `II_ERROR`, all accepted probability-list arities, and empty groups | Exact equivalence to omission across all surfaces and no noise mechanism in the DEM | Semantic invariant |
| `pfm3-contract-depolarization` | `DEPOLARIZE1(0.6)` and `DEPOLARIZE2(0.75)`, including empty groups | One-qubit rates `I=0.4,X=Y=Z=0.2`; two-qubit rates `II=0.25,nonidentity=0.75` | Statistical |
| `pfm3-contract-correlated-errors` | `E`, `ELSE_CORRELATED_ERROR`, empty first or else branches, and legal Pauli lists | Chain rates `no-error=0.3,first=0.2,else-one=0.2,else-two=0.3`; reference, converter, and flow paths remain unchanged | Statistical |
| `pfm3-contract-heralded-noise` | `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1`, including empty groups | Pinned erase rates `no-herald=0.9,I=X=Y=Z=0.025`; pinned channel rates `no-herald=0.45,I=0.05,X=0.10,Y=0.15,Z=0.25`; offset-by-two shards repeat both distributions | Statistical |
| `pfm3-contract-annotations` | `DETECTOR`, `OBSERVABLE_INCLUDE`, `TICK`, `QUBIT_COORDS`, and `SHIFT_COORDS`, including empty legal groups | Exact record indexing, coordinate shifts, observable behavior, unchanged quantum state, analyzer declarations, and flow annotations | Exact |
| `pfm3-contract-classical-controls` | Forward `CX` and `CY`, symmetric `CZ`, reverse `XCZ` and `YCZ`, every qubit, record, and sweep role pair, and empty groups | Generated positive, no-op, and negative directional matrix; sweep order and all-false omission behavior; consistent typed rejection on every semantic surface | Semantic invariant |
| `pfm3-contract-control-flow` | Empty and nested `REPEAT` blocks | Folded and unrolled exact equivalence across all applicable engines; contract marks the synthetic gate as not applicable while circuit traversal executes the block | Semantic invariant |

## Exact Provenance Split

The 19 additional exact subcases are not new product scope; they replace planned test-family aggregation with direct pinned-test provenance:

- Fixed-tableau general-circuit execution: `pfm3-contract-fixed-tableau-general-circuit`.
- Noisy reset bases: `pfm3-contract-measure-reset-x`, `pfm3-contract-measure-reset-y`, and `pfm3-contract-measure-reset-z`.
- Pair-measurement inversion: `pfm3-contract-pair-measurement-inversion`.
- Grouped and rejected phase products: `pfm3-contract-spp-multiple` and `pfm3-contract-spp-rejection`.
- General-circuit stochastic dispatch: `pfm3-contract-pauli-noise-general-circuit`, `pfm3-contract-pauli-channels-general-circuit`, `pfm3-contract-depolarization-general-circuit`, and `pfm3-contract-correlated-errors-general-circuit`.
- Heralded channel and offset statistics: `pfm3-contract-heralded-channel`, `pfm3-contract-heralded-erase-offset`, and `pfm3-contract-heralded-channel-offset`.
- Annotation coordinates and tags: `pfm3-contract-annotation-coordinates` and `pfm3-contract-annotation-tags`.
- Classical-control rejection, feedback directionality, and omitted sweep data: `pfm3-contract-classical-control-rejection`, `pfm3-contract-classical-control-feedback`, and `pfm3-contract-classical-control-no-sweep-data`.

## Statistical Contract

The machine-readable source of truth must store each bucket name together with its expected probability.
Every statistical case uses 100,000 shots, its ledger seed, a six-sigma multiplier, an absolute probability floor of 0.01, and a familywise false-positive budget of `0.000001`.
For each bucket, the test accepts an observed rate only when its absolute difference from the expected rate is at most `max(0.01, 6 * sqrt(p * (1 - p) / 100000))`.
Ledger validation uses `statrs` to calculate the discrete two-sided binomial rejection probability at the resulting integer boundaries and rejects a plan whose union bound exceeds the declared familywise budget.
Tests and ledger validation must consume the same source-owned probability catalog so a label-only ledger cannot drift from executable expectations.
Exact-tail evaluation occurs only after the frozen semantic digest matches and the aggregate catalog stays within 128 bucket evaluations, so hostile mutated ledgers cannot force unbounded statistical work.

## Rejection Contract

The parser intentionally accepts anti-Hermitian `MPP`, `SPP`, and `SPP_DAG` target products because target syntax is valid.
The measurement sampler, reference sampler, detection converter, detector frame, detection sampler, error analyzer, and flow generator must all reject these products with a typed error whose message identifies the anti-Hermitian product.

Directional classical controls must reject quantum-to-classical target roles for asymmetric gates and must reject classical-only pairs except for the symmetric `CZ` no-op matrix.
The generated test must enumerate every `unsupported_shape` decision from the canonical contract and prove that the owning production surface fails closed.

## Resource Contract

Final execution tests must preserve these boundaries:

- Fixed-tableau and ordinary gate dispatch must not allocate per gate in an execution loop merely to consult the semantic contract.
- Identity-noise and empty-target no-op cases must not add state proportional to target count beyond ordinary circuit storage.
- Sweep behavior retains the completed `pf3-analyze-errors-sweep` low and maximum-ID allocation comparison; no execution test may allocate through the numeric sweep ID.
- Statistical tests materialize at most their declared 100,000 small records per surface, avoid duplicate record vectors, and use one 20-slot fixed bucket counter.
- The final implementation must not add a production contract lookup unless a semantic defect requires it; if production dispatch remains unchanged, no mixed-contract benchmark row is required.

## Oracle Plan

Add one implemented Cargo-test proxy row for each `pfm-b2-gate-contract-*` id.
Each row must select exactly one full Rust test name with `--exact`, cite the pinned Stim v1.16.0 source named by the ledger, and describe the exact, structural, rejection, or statistical assertions performed inside that test.
The existing broad PF3 rows remain supporting historical evidence and must not substitute for the 37 independently selectable final shards.

## Benchmark Disposition

No per-gate microbenchmark is added.
The `pf3-gate-semantic-wide` representative row remains report-only for fixed-tableau, measurement, MPP, MPAD, SPP, noise, annotation, classical-control, and repeat cases, but its former heterogeneous aggregate is replaced by separate sampler execution, reference sampling, converter compilation, ordinary detection sampling, forced detector-frame sampling, error analysis, and flow-generation measurements.
`pf3-analyze-errors-sweep` remains the allocation and maximum-sweep-ID evidence row.
PFM-B2 fixed production defects in controlled-Pauli dispatch, heralded sampler and detection-reference handling, empty-target analyzer no-ops, anti-Hermitian analyzer validation, and per-shot sweep-reference scratch allocation, but no production path consults the semantic contract table.
The existing `pf3-gate-semantic-wide` and `pf3-analyze-errors-sweep` rows therefore remain the representative performance and resource evidence; the mixed-contract trigger does not fire.
If a future production compile or execution path starts consulting the semantic contract, add a mixed-family compile-and-execute row and classify it before timing.

The clean timing command to rerun is:

```sh
just bench::compare --only pf3-gate-semantic-wide --only pf3-analyze-errors-sweep --warmup --measurement-runs 3 --baseline target/benchmarks/pfm-b2-closure-baseline/baseline.json --report target/benchmarks/pfm-b2-closure-compare
```

The new report must record all seven gate submeasurements plus the three analyzer-sweep submeasurements, leave the report-only row median empty, and render normalized rates in the report-only submeasurement table.
Allocation-tracked reports must be stored under `target/benchmarks/pfm-b2-closure-gate-allocations` and `target/benchmarks/pfm-b2-closure-sweep-allocations`.
The execution-only fixed-tableau regression separately proves warmed dispatch allocation does not scale with repetitions, and the sweep-conversion regression proves the converter adds no per-shot scratch allocation or record-copy buffer.
Both rows remain `report-only` and `contract-only`; no aggregate Stab/Stim ratio or beta-gate claim is made.

## Completion Checklist

- [x] Source-owned statistical bucket probabilities and exact-tail plan validation are implemented.
- [x] All 37 exact Cargo selectors exist and resolve independently.
- [x] Generated positive, no-op, annotation, lower-then-execute, not-applicable, and unsupported decisions have production-path evidence.
- [x] Deterministic expected values are exact and stochastic expected values use the frozen plans.
- [x] Every final ledger case is `implemented` with an existing test, oracle signature, and honest benchmark reference.
- [x] All 37 oracle rows pass through `just oracle::run --milestone PF3`.
- [x] The low and maximum sweep-ID allocation evidence remains clean.
- [ ] Documentation agrees on active and deferred scope.
- [x] Milestone-audit implementation, evidence, benchmark, and resource findings are fixed; the exact-provenance loophole is resolved in `milestone-spec-gaps.md`.
- [ ] GPT-5.6/max full-code-review findings are fixed.
