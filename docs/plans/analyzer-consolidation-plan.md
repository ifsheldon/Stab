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
- Stage 1: this document exists (gate for Batch C code); matrix fixtures not yet captured.
- Stage 2: blocked on Stage 1 and the baseline-divergence diagnosis.
- Stage 3: blocked on Stage 2.
- Stage 4: Pass 2 (Batch D).
