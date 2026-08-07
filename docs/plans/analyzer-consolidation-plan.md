# Analyzer Consolidation Plan

Stage-gated execution record for WS2b of [post-review-remediation-plan.md](post-review-remediation-plan.md), per maintainer decision D6: retire the forward `Analyzer` and `GaugeTracker` so `SparseReverseFrameTracker` becomes the single sensitivity-propagation engine.
Each stage below is a merge gate; a stage's code may not merge until the previous stage's exit criteria are recorded here.

## Why The Forward Engine Is Being Retired

Stim's DEM semantics are defined by a reverse-in-time pass: a detector or observable's sensitivity region extends backward from its declaration to the previous collapse.
The forward formulation gets exactly this wrong in executed witnesses (Pauli-target `OBSERVABLE_INCLUDE` sensitivity applied forward instead of backward), produces different error content depending on `fold_loops` because only the folded path uses the correct reverse tracker, and keeps three owners of the sensitivity-propagation invariant in sync by hand (forward analyzer, `GaugeTracker`, reverse tracker).
The crate already contains a faithful, vendor-diffed port of Stim's reverse tracker (`crates/stab-analysis/src/sparse_rev_frame_tracker.rs`) used by the folded path, so consolidation deletes the drift class instead of patching it.

## Known Baseline Divergence (must be diagnosed before Stage 2)

During WS2a verification, the non-decompose `analyze_errors` DEM for a generated rotated surface code (distance 3, rounds 3, `--after_clifford_depolarization 0.001`) diverged from pinned Stim on BOTH fold paths, independent of the decomposition change (verified against the pre-port code).
Two distinct symptom classes were observed: seventeenth-significant-digit probability drift on shared error classes (accumulation-order or combination-formula differences) and real content differences (Stab reports `error(0.00053...) D0 D8` where pinned Stim reports `error(0.00027...) D0 D8` plus a separate `error(0.00013...) D1 D5 ^ D4` class).
Reproduce with: `target/stim-v1.16-probe/out/stim gen --code surface_code --task rotated_memory_z --distance 3 --rounds 3 --after_clifford_depolarization 0.001`, then `analyze_errors` on both binaries with and without `--fold_loops`.
Because the folded path already uses the reverse tracker, this divergence is not explained by the forward engine alone; the diagnosis (channel splitting, disjoint-probability combination, or propagation ordering) must be recorded here before Stage 2 code lands, and the fix belongs to whichever stage owns the diverging semantics.
Stage 1's matrix enumeration sharpened the scope: exactly 18 of 63 entries diverge, and they are precisely the two Pauli-include review witnesses (the known forward-engine direction bug) plus every entry whose noise setting contains a DEPOLARIZE channel (`--after_clifford_depolarization` everywhere; `--before_round_data_depolarization` wherever the circuit produces multi-detector combinations), on both fold paths.
Flip-probability and reset-flip settings match byte-exactly everywhere, so the divergence lives in the depolarizing-channel combination arithmetic.
The local-decomposition suspicion was falsified by source reading: the vendor gates that on `decompose_errors` exactly like Stab.
Probing pinned doubles against candidate float expressions established two site-specific rounding forms: cross-instruction map merges follow `old * (1 - p) + (1 - old) * p` (error_analyzer.cc:1088, verified by a five-instruction distinct-argument chain), while slots sharing one symptom class inside a single `add_error_combinations` call combine like `l + r - 2lr` before the class reaches the map (verified by the four-slot DEPOLARIZE2 case, where the chained vendor form is provably wrong and the pre-combined form is byte-exact).
Stab's reverse path now carries both site-split forms (`xor_probability` for map merges, `within_call_xor_probability` for the new within-call subtotal), which healed the two color-code `brd` fold entries; the residual sixteen fold divergences localize to pinned Stim's loop-folding machinery, where cross-round DEPOLARIZE merges round differently than any encounter-order composition of either form (demonstrated by black-box probes that exhaust the composition space), so the next diagnosis step is an instrumented local build of the pinned Stim source rather than further CLI probing.
The forward analyzer keeps its pre-consolidation `l + r - 2lr` rounding everywhere (`merge_independent_probability_legacy`) because it matches pinned Stim on a different circuit subset than the vendor form, no single global formula can match both, and the engine is deleted at Stage 4; the matrix harness therefore tracks fold and nofold divergences separately (sixteen versus eighteen entries).
A separate accept-divergence was catalogued while re-pinning tests: Stab's exact Newton conversion accepts `PAULI_CHANNEL_1(0.1792, 0.1008, 0.2592)` without `approximate_disjoint_errors` where pinned Stim rejects it; this pre-existing behavioral difference must be resolved (or documented as a deliberate extension) before Stage 3 declares diagnostics parity.

## Stage 0: Reverse-Family Completion

The reverse implementation refuses exactly three instruction families, and `FoldedAnalyzer` silently falls back to the forward engine for them (`contains_unsupported_reverse_fold_instruction`, `crates/stab-analysis/src/circuit_to_dem/reverse_fold.rs`; fallback in `crates/stab-analysis/src/circuit_to_dem/folded.rs`): `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`.
Heralded-noise circuits therefore always use the forward engine today, even with `fold_loops` enabled.

Tasks:

1. Capture pinned-Stim `analyze_errors` fixtures for the three families, standalone and inside repeat blocks, with `fold_loops` on and off.
2. Implement reverse-tracker support for the three families, mirroring the vendor `undo_ELSE_CORRELATED_ERROR`, `undo_MPAD`-adjacent heralded record handling, and heralded channel semantics.
3. Remove the three families from the fallback predicate so the fold path stops falling back.

Exit criteria: the three-family fixtures byte-match pinned Stim with `fold_loops` enabled, including inside repeat blocks; the fallback predicate no longer names them; regressions fail against the fallback behavior where output previously came from the forward engine.

## Stage 1: Equivalence Matrix And Fixture Capture

The full equivalence matrix, captured as committed pinned-Stim byte-exact DEM fixtures BEFORE any Stage 2 engine change (checkable from history):

1. Generated families: `repetition_code:memory`, `surface_code:rotated_memory_z`, `surface_code:rotated_memory_x`, `surface_code:unrotated_memory_z`, and `color_code:memory_xyz`, each at two distances (3 and 5, rounds equal to distance), each with no noise and with each of `--after_clifford_depolarization`, `--before_round_data_depolarization`, `--before_measure_flip_probability`, and `--after_reset_flip_probability` at 0.001.
2. Unpaired Pauli-target `OBSERVABLE_INCLUDE` circuits in both fold modes, including the executed review witness pair (`OBSERVABLE_INCLUDE(0) Z0` before versus after `X_ERROR(0.25) 0`).
3. Gauge circuits under `--allow_gauge_detectors`, including the reset, measurement-prelude, and multi-round variants already pinned by the M10 gauge fixtures.
4. Feedback (`CX rec[-1] q` and `XCZ`/`YCZ` record forms) and sweep-target circuits within the pinned analyzer sweep boundary.
5. MPAD circuits standalone and inside repeat blocks.
6. The Stage 0 heralded and `ELSE_CORRELATED_ERROR` families.
7. The existing pf6, pfm-b3, and pfm-b5 fixture inputs, reused as matrix entries.

Every entry is captured with `fold_loops` on and off and with `--decompose_errors` off (decomposition equivalence is already owned by the WS2a rows; byte-exact decomposed surface-code rows land after the baseline divergence above is fixed).

## Stage 2: Dual-Engine Differential

1. Implement the non-folded path on `SparseReverseFrameTracker` (fold detection disabled, loops unrolled under the existing expansion budget) behind an internal selection seam, leaving the forward engine in place.
2. Add a differential test running both engines across the whole matrix, asserting byte-equal DEM output except for entries reproducing the known forward-engine bugs, listed explicitly with witness identities.

Exit criteria: differential green across the matrix; the exception list matches the review witnesses exactly and nothing else.

## Stage 3: Flip And Verify

1. Make the reverse-based path the only public path; assert the matrix byte-matches the pinned-Stim fixtures, including the previously-buggy entries.
2. Verify fold/no-fold equality across the matrix and the review witness pair in both fold modes.
3. Enumerate every diagnostics-text change as a compatibility surface before merging.

## Stage 4: Delete And Split (Pass 2, Batch D)

1. Delete `GaugeTracker`, the forward sensitivity machinery, and the fallback seam.
2. Split modules so no file crosses the 1200-line policy; delete the differential seam.
3. Preserve the public analyzer options surface with MIGRATING-0.2.md notes for any removed public item.

## Stage Ledger

- Stage 0: complete. The reverse tracker analyzes `ELSE_CORRELATED_ERROR` chains (telescoping disjoint components exactly like the vendor `correlated_error_block`), `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1` (three-basis disjoint combinations over the popped herald record and the qubit's Z/X sensitivity regions, vendor slot order `hi, hz, hx, hy`); the fold-path fallback and its predicate are deleted; ten binary comparisons (five fixtures, both fold modes) byte-match pinned Stim; the `reverse_heralded_families` suite pins the outputs and the requires-approximate error classes across fold modes; oracle rows `m10-analyze-errors-{heralded-erase,heralded-pauli,else-chain,else-chain-repeat}-fold` are committed and pass; the previously-diverging folded heralded loop output (`repeat 2 { error(0.125) D0; shift_detectors 1 }`) now byte-matches pinned Stim where the old fallback flattened it; and the DEM printer emits pinned Stim's blank line inside empty repeat bodies, a byte divergence the heralded folding made reachable.
- Stage 1: complete. Sixty-three matrix entries are committed under `crates/stab-analysis/tests/consolidation_matrix/` (fifty generated-family circuits across five code families, two distances, and five noise settings, plus the Pauli-include witness pair, gauge, feedback, sweep, MPAD, and the Stage 0 families), each with pinned-Stim `nofold` and `fold` DEM captures; the `consolidation_matrix` harness asserts the forty-five currently-matching entries byte-match and pins the eighteen known divergences by name, failing if any divergence heals or appears unlisted; the pf6, pfm-b3, and pfm-b5 fixture inputs are reused in place by the oracle corpus rather than duplicated here.
- Stage 2: blocked on the depolarizing-channel divergence diagnosis above.
- Stage 3: blocked on Stage 2.
- Stage 4: Pass 2 (Batch D).
