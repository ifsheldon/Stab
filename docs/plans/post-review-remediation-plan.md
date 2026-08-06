# Post-Review Remediation Plan

## Summary

This plan schedules the fixes for every confirmed finding from the August 2026 full code review of the Stab workspace.
The review followed `.agents/skills/full-code-review/SKILL.md`, covered all product crates, ops crates, test-support crates, workflows, and the operational command surface, and verified each significant finding against the code; the highest-severity findings were additionally reproduced by executing the pinned Stim v1.16.0 binary (`vendor/stim`, commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`) against the built `stab` binary.
The review found no P0 findings, eleven P1 findings, and a set of P2 and P3 findings that cluster into a small number of themes: two semantic bugs in `stab-engine` detector paths, an incorrect original forward analyzer and decomposition pass in `stab-analysis`, a batch of narrow `.stim`/CLI grammar divergences, one release-pipeline API misuse, duplicated ownership of single invariants across layers, and CPU-shaped hostile-input budget gaps in a codebase whose memory budgets are otherwise rigorous.
Work is organized into workstreams `WS1` through `WS8` plus a bookkeeping phase `P0`, executed as two dependency-gated passes plus a triggered backlog per the execution overlay below: Pass 1 closes release-blocking correctness and safety defects, Pass 2 performs lean-core consolidation by deleting duplicate owners, and everything else waits in the backlog behind explicit promotion triggers.
Sizes are S (about a day or less), M (a few days), and L (a week or more of focused work).
Each workstream states its deliverables and machine-checkable success criteria; a workstream is not done until every success criterion holds from a clean committed revision.

Use `docs/plans/lessons-learned.md` as the guardrail.
Every fix that changes implemented behavior, public APIs, CLI flags, file formats, or workflows must update the matching documentation, checklist rows, and oracle fixtures in the same change set, and must not weaken an existing gate to pass.
Upstream file names are not acceptance criteria; every claim below names exact behaviors, comparators, and tests.

## P0.0: Release-Authorization Freeze (Gate 0, Documentation Only)

`docs/plans/GOAL.md` currently states that the final reviews found no P0 or P1 issue and holds Milestone A9 at pre-evidence, staged toward qualification evidence, crates.io publication, tag creation, and release; this plan records eleven confirmed P1 findings against the same revision line.
Known P1 findings and active release authorization cannot coexist in separate sources of truth, so this freeze is the first gate of the plan and must merge before any product fix.

Tasks:

1. Set `docs/plans/GOAL.md` to a reopened remediation state that names this plan, and remove or supersede its "no P0 or P1 issue" claim.
2. Explicitly prohibit, until this plan's Pass 1 closes: new A9 correctness or performance evidence production, completion checkpoints, package preflight, crates.io publication, `v0.2.0` tag creation, and GitHub draft creation.
3. Flip the over-claiming rows in `docs/stab-feature-checklist.md` to reopened status and regenerate `docs/qualification-status.md`.
4. Create `docs/plans/post-review-remediation-progress-report.md` and record the exact remediation base commit SHA and the freeze state.

Success criteria:

1. `docs/plans/GOAL.md` no longer authorizes release actions and names this plan as the active contract; the "no P0 or P1 issue" sentence is gone.
2. The reopened checklist rows and regenerated dashboard are committed, and the progress report exists with the base SHA, all in one documentation-only change set that precedes every product fix in history.

## Evidence Rules For This Plan

- Every behavior fix invalidates current CQ and PQ evidence for the affected feature; per `docs/AGENTS.md`, a correctness or performance inventory digest change makes current evidence historical until the affected tiers are rerun from a clean committed revision.
- Regenerate inventories once per merged batch (`just qualification::correctness-regenerate`, `just bench::qualification-regenerate`) rather than per fix, and run one `--tier full` qualification rerun at the end of the plan rather than per fix.
- Do not promote correctness or performance claims for a reopened surface until its fixes, oracle fixtures, and fresh evidence all exist.

## Resolved Decisions

These decisions were reviewed and accepted by the maintainer on 2026-08-05; each gates one or more fixes below and must be recorded in the matching docs when its fix lands.

- D1 `.dem` float precision: keep the current 34-significant-digit printing (`crates/stab-model/src/dem.rs:48`) and document Linux AArch64 pinned Stim v1.16.0 as the frozen byte baseline for `.dem` output; record that x86-64 C++ Stim prints 19 digits so byte-level oracle comparison is only valid on the frozen baseline platform.
- D2 broken pipe: adopt exact Stim parity by re-raising `SIGPIPE` so a closed downstream pipe terminates `stab` silently with status 141; keep error reporting for genuine output I/O failures.
- D3 `E`/`ELSE_CORRELATED_ERROR` target strictness: match Stim v1.16.0 by accepting combiners and inverted Pauli targets on these gates (accept-and-ignore semantics per `vendor/stim/src/stim/circuit/circuit_instruction.cc:249-262`).
- D4 `sample_dem` observable routing flags: keep `--append_observables`/`--prepend_observables` as Stab extensions, mark both `hide = true`, name both flags explicitly in the checklist row, and keep the `stab help sample_dem` topic in sync.
- D5 `convert` input cap: keep the documented 64 MiB hostile-input cap until the shared streaming record reader from WS5 lands, then convert `convert` to streaming and drop the whole-input cap without losing bounded-memory guarantees.
- D6 forward-analyzer retirement: retire the forward `Analyzer` and `GaugeTracker` in favor of `SparseReverseFrameTracker` as the single propagation engine (WS2b); the tactical forward patch (retroactive include toggling plus gauge symptom rewriting) is rejected as spending effort on an engine scheduled for deletion.
- D7 execution shape: run this plan as two dependency-gated passes plus a triggered backlog (see the execution overlay); the release freeze (P0.0) precedes everything, and calendar-based sequencing is replaced by batch gates.

## Amendment Record

- 2026-08-05, revision 1: amended after an independent second review and a verification pass converged.
  Adopted: the P0.0 release-authorization freeze; the MPAD task correction (target validation already exists, only counting semantics and regression tests remain); WS2b Stage 0 for the three reverse-analyzer fallback families; a mock-fidelity note on the WS4 draft-endpoint item; dependency-gated batches replacing calendar weeks; the two-pass execution overlay with a triggered backlog keeping the MPP quadratic and recursive-circuit abort in Pass 1; explicit statistical-test contracts; and expanded progress-report metrics.
  Rejected from the second review's first round, with verification evidence: the attached-combiner defect description (attached forms already tokenize; the defect is spaced forms), the removal of the WS4 draft-endpoint task (the endpoint defect is real and the existing mock masks it), and a nonexistent toolchain-standardization item.

## Non-Goals And Deferrals

- No new Stim surfaces are implemented under this plan; it only repairs and consolidates already-implemented behavior.
- `explain_errors`, full ErrorMatcher provenance, Python bindings, WASM, diagrams, and GPU work remain deferred per the roadmap.
- The exact uniform `Tableau::random` sampler, the columnar bit-packed `Tableau` rework, and the SIMD `pauli_right_multiply_block` kernel are recorded here as benchmark-gated or scheduled architecture items, not committed deliverables of this plan.
- This plan does not change benchmark acceptance thresholds, statistical plans, or qualification tier definitions.
- Fixing every 900-plus-line watch-list file is out of scope; only splits that fall out naturally from WS2 and WS5 ownership moves are included.

## Execution Overlay: Two Passes And A Backlog

Pass 1 (release-blocking correctness and safety):

- P0.0 release freeze, then P0 bookkeeping.
- WS1 engine semantic fixes.
- WS2a decomposition correctness.
- WS2b Stages 0 through 3 (reverse-family completion, fixtures, dual-engine differential, public-path switch).
- WS3 grammar and CLI compatibility fixes.
- WS4 release-tooling fixes.
- WS6 item 1 (MPP quadratic) and WS6 item 7 (recursive `Circuit` drop/clone/equality abort), which are confirmed hostile-input and safety defects with witnesses, not speculative performance work.

Pass 2 (lean-core consolidation, no intended behavior changes):

- WS2b Stage 4 (forward-engine and `GaugeTracker` deletion after parity).
- WS5 ownership consolidation.
- Net implementation deletion reported by crate and owner in the progress report.

Backlog (deferred behind promotion triggers):

- WS6 items 2 through 6 and 8, WS7, and WS8.
- Promotion triggers, any one of which moves an item into the active pass: a reproduced timeout or resource-exhaustion witness; a failed correctness prerequisite that the item blocks; a measured benchmark bottleneck on a qualification path; a CI or release workflow failure attributable to the item; a required pre-1.0 API break that the item owns.
- Recommended early promotions when their lanes produce Pass 1 evidence: the compat-corpus double-hex test fix and the oracle statistical stderr-class assertion (WS8), because they strengthen the evidence Pass 1 relies on.

## Progress Reporting

- Keep one rolling progress report at `docs/plans/post-review-remediation-progress-report.md`, updated at each batch completion.
- Record per update: the remediation base revision and current head; the release-freeze state; per finding, its owner, witness, implementing commit, and evidence status; affected evidence invalidated and regenerated; oracle rows added; production lines added and deleted by crate; duplicate owners removed; the exact commands run with output digests; residual risks and explicitly deferred backlog items; and the criteria for returning each checklist row from reopened to done.
- WS2 additionally maintains `docs/plans/analyzer-consolidation-plan.md` (written before WS2b Stage 1 code) and records its stage gates there.
- Use `.agents/skills/milestone-audit` before declaring WS1, WS2, or WS3 complete, because those workstreams carry compatibility claims.

## WS1: Engine Semantic Corrections (M)

These are the two most valuable fixes in the codebase, plus ride-along items in the same files.
No dependencies beyond Gate 0; start immediately.

### Product-measurement collapse in the detector frame path

`crates/stab-engine/src/detection/frame.rs:322-337` computes a product measurement's outcome deviation from all terms but then collapses by randomizing only the first term's frame bit via `randomize_measured_basis`.
Stim multiplies the frame by the entire measured product (`vendor/stim/src/stim/simulators/frame_simulator.inl:870-882`), so Stab's detector statistics are silently wrong for heralded-noise circuits containing MXX/MYY/MZZ/MPP; the frame path is selected for every circuit with `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`, or Pauli-target `OBSERVABLE_INCLUDE` (`crates/stab-engine/src/detection/requirements.rs`).

Tasks:

1. In `measure_pauli_product_terms`, draw one random bit per product measurement and, when set, XOR the full product into the frame (X terms via `xor_x_bit`, Y via both, Z via `xor_z_bit`); keep the current single-term behavior, which already matches Stim.
2. Add a frame-path regression asserting a deterministic detector across anticommuting-then-commuting products: `HERALDED_ERASE(0) 2` / `R 0 1` / `MXX 0 1` / `MZZ 0 1` / `DETECTOR rec[-1]` must never fire across at least 10k seeded shots.
3. Add a correlation-sensitive statistical test over a two-detector joint distribution, because the existing marginal-only tests provably cannot see this bug class.

### Reference-sample flip semantics

`crates/stab-engine/src/sampling/measurement_flip.rs:5-18` applies `p == 1.0` measurement flips in `ReferenceSample` mode, while Stim's reference sample is strictly noiseless because `aliased_noiseless_circuit` drops result-flip probabilities (`vendor/stim/src/stim/circuit/circuit.cc:791`).
This inverts `detect`, `m2d`, and `--skip_reference_sample` outputs per shot for measurements with probability exactly 1.

Tasks:

1. Make `ReferenceSample` mode ignore flip probabilities entirely (return false); keep `MPAD`'s target-encoded value.
2. Add `M(1) 0` / `DETECTOR rec[-1]` oracle fixtures through `reference_sample`, `detect`, `m2d`, and skip-reference sampling, plus p equal to 0 and p in (0, 1) cases across the noisy measurement families.

### Ride-along engine items

1. Make the free function `count_determined_measurements` (`crates/stab-engine/src/sampling/mod.rs:207-214`) return the storage error via `try_count_determined_measurements` and delete the panicking wrappers; a parseable hostile circuit must not panic a public API.
2. Align `count_determined_measurements` semantics with Stim (`vendor/stim/src/stim/util_top/count_determined_measurements.inl`): ignore measurement flip arguments and reject MPAD/heralded gates; pin with an oracle-derived test and record the change in `docs/MIGRATING-0.2.md`.
3. MPAD counting semantics: target validation to {0, 1} already exists via `TargetRule::MeasurementPads` (`crates/stab-model/src/gate/mod.rs:405-407`), so do not add new validation; instead make public qubit counting exclude MPAD targets through gate target-role metadata, consolidate it with the private `count_simulated_qubits` (`crates/stab-model/src/circuit/counts.rs:64`) so one owner remains, and add standalone and repeat-nested MPAD regression tests, matching `vendor/stim/src/stim/circuit/circuit_instruction.cc:64-69`.
4. In the determinism-count path (`crates/stab-engine/src/sampling/execute.rs:56-67`), either condition reset correction on the physical outcome and push a herald placeholder record, or document the sign-invariance argument that makes the current shape safe; do not leave the divergence from the execution path uncommented.

### WS1 success criteria

1. The heralded MXX-then-MZZ witness detector fires zero times across at least 10k seeded shots, and the test fails when run against the pre-fix collapse logic.
2. A joint two-detector distribution test distinguishes whole-product collapse from first-term collapse under the statistical test contract below and passes only for the fixed implementation.
3. `M(1) 0` / `DETECTOR rec[-1]` fixtures byte-match pinned Stim for `detect` (01 and dets formats), `m2d`, `reference_sample`, and `--skip_reference_sample` sampling, as committed exact-output oracle rows.
4. No public function in stab-engine panics on any parseable circuit: the huge-qubit-id witness (`M 16000000`) returns a typed storage error through every public entry point, covered by a test.
5. `count_determined_measurements` agrees with pinned Stim on a case matrix including `R 0` / `M(0.5) 0` (determined) and rejects MPAD/heralded inputs with a typed error; the migration note exists.
6. A regression test pins the already-implemented MPAD target validation (targets outside {0, 1} rejected at construction and parse), public qubit counting excludes MPAD targets through one metadata-driven owner shared with the simulated count, and standalone plus repeat-nested MPAD counting tests pass.
7. `cargo test -p stab-engine -p stab-model` passes; clippy and fmt clean; checklist rows for detect/m2d/heralded sampling move from reopened back to implemented (qualification still pending the end-of-plan rerun).

## WS2: Analyzer Consolidation (L)

Five P1 findings, the gauge P2, and the triple-ownership P2 share one root cause: the forward `Analyzer` (`crates/stab-analysis/src/circuit_to_dem.rs`) and the global decomposition pass (`crates/stab-analysis/src/circuit_to_dem/decompose.rs`) are original algorithms, while the crate already contains a faithful, vendor-diffed port of Stim's reverse tracker (`crates/stab-analysis/src/sparse_rev_frame_tracker.rs`) used by the folded path.
Per D6, the forward engine and `GaugeTracker` will be retired; write `docs/plans/analyzer-consolidation-plan.md` before starting WS2b Stage 1 code, including the full equivalence test matrix.

### Rationale for retiring the forward engine

Stim's DEM semantics are defined by a reverse-in-time pass: a detector or observable's sensitivity region extends backward from its declaration to the previous collapse.
The executed witnesses show the forward formulation getting exactly this wrong: Pauli-target `OBSERVABLE_INCLUDE` sensitivity is applied forward instead of backward (`crates/stab-analysis/src/circuit_to_dem.rs:566-577`), and the same circuit produces different error content depending on `fold_loops` because the folded path uses the correct reverse tracker.
Fixing the forward engine in place requires retroactive-mutation machinery (toggling observables into already-pending errors at include time, rewriting pending symptoms in the gauge-fixed basis for `remove_gauge` parity, adding per-qubit indexes to remove the structural quadratic) that incrementally rebuilds what the reverse tracker already does, while keeping three copies of Stim gate semantics in sync forever.
The crate currently has three owners of the sensitivity-propagation invariant (forward analyzer, `GaugeTracker`, reverse tracker), and this duplication has already produced two confirmed semantic skews, so the drift class is demonstrated rather than hypothetical.
Retiring the forward engine deletes the whole class: one engine, one place to apply Stim fixes, and the quadratic disappears because the reverse pass touches each error once with per-qubit sets.

### WS2a: Port Stim's within-problem decomposition (M, engine-independent, land first)

`decompose_remaining` (`crates/stab-analysis/src/circuit_to_dem/decompose.rs:133-165`) searches over the entire model's known components with a growable XOR state, no term cap, and no reuse mask, producing decompositions with duplicate self-cancelling components; `remnant_decomposition` (`decompose.rs:168-182`) handles only one known component plus a graphlike remainder and rejects circuits Stim accepts; the search is the only unbudgeted one in the crate.

Tasks:

1. Replace both with a port of Stim's `brute_force_decomp_helper` and `decompose_and_append_component_to_tail` (`vendor/stim/src/stim/simulators/error_analyzer.cc:1335-1470`): within-problem terms only, used-term mask, pairs before singles, observable-mask tracking, greedy known pairs then singles then a remnant of at most two missed detectors, and the 64-term cap.
2. Skip zero-probability entries when building known graphlike components, matching `error_analyzer.cc:1495-1497`.
3. Match Stim's over-64-term rejection and record it in the checklist.
4. Route intra-channel insertions on the non-folded path through the existing `reverse_fold/local_decomposition.rs` logic so `fold_loops` stops changing decomposition output.
5. Regressions: the two executed review witnesses (duplicate-component decomposition; six-detector remnant acceptance), plus a byte-exact pinned-Stim DEM for a depolarizing rotated surface code with `--decompose_errors` under both `fold_loops` values.

### WS2a success criteria

1. The four-error hyperedge witness decomposes to exactly `D0 D1 ^ D2` (byte-equal to pinned Stim), and the six-detector remnant witness is accepted as `D0 D1 ^ D2 D3 ^ D4 D5`; both are committed regression tests that fail against the old search.
2. Byte-exact DEM equality with pinned Stim for the depolarizing rotated surface code at distances 3 and 5 with `--decompose_errors`, with `fold_loops` both on and off, as committed exact-output oracle rows.
3. A structural invariant test proves every decomposition output: components XOR back to the original problem, no component appears twice, and every non-remnant component exists in the known set.
4. Problems with more than 64 terms are rejected with Stim's error class, and zero-probability known entries are excluded, both covered by tests.
5. The decomposition search allocates no candidate list proportional to the whole model (verified by the within-problem port shape) and `fold_loops` no longer changes decomposition output anywhere on the equivalence matrix.
6. `--ignore_decomposition_failures` and `--block_decomposition_from_introducing_remnant_edges` behavior matches pinned Stim on a small fixture per flag.

### WS2b: Retire the forward engine (L)

Execution is staged so the old engine is deleted only after the new path is proven byte-equivalent; each stage is a merge gate recorded in `docs/plans/analyzer-consolidation-plan.md`.
Stages 0 through 3 belong to Pass 1 (correctness); Stage 4 belongs to Pass 2 (deletion).

Stage 0 (reverse-family completion):

1. The reverse implementation currently refuses exactly three instruction families, and `FoldedAnalyzer` silently falls back to the forward analyzer for them (`contains_unsupported_reverse_fold_instruction`, `crates/stab-analysis/src/circuit_to_dem/reverse_fold.rs:770-780`; fallback at `crates/stab-analysis/src/circuit_to_dem/folded.rs:21-23`): `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`.
2. Consequence worth recording: heralded-noise circuits always use the forward engine today, even with `fold_loops` enabled, so the reverse engine cannot become the only engine until these families are ported.
3. Capture pinned-Stim `analyze_errors` fixtures for the three families (including inside repeat blocks), implement reverse-tracker support for them, and make the fold path stop falling back for these families.

Stage 1 (plan and fixtures):

1. Write `docs/plans/analyzer-consolidation-plan.md` with the full equivalence matrix: generated-code families (repetition, rotated and unrotated surface, color) at two distances with all four noise channels and with no noise, unpaired Pauli-include circuits in both fold modes, gauge circuits under `--allow_gauge_detectors`, feedback and sweep circuits, MPAD circuits, and the heralded and `ELSE_CORRELATED_ERROR` families from Stage 0, plus the existing pf6/pfm-b3/pfm-b5 fixture inputs.
2. Capture pinned-Stim byte-exact DEM outputs for every matrix entry as committed fixtures before any further engine change.

Stage 2 (dual-engine differential):

1. Implement the non-folded path on `SparseReverseFrameTracker` (fold detection disabled, loops unrolled under the existing expansion budget) behind an internal selection seam, leaving the forward engine in place.
2. Add a differential test that runs both engines across the whole matrix and asserts byte-equal DEM output except for the entries that reproduce the known forward-engine bugs, which must be listed explicitly with their witness identities.

Stage 3 (flip and verify):

1. Make the reverse-based path the only public path; assert the matrix now byte-matches the pinned-Stim fixtures, including the previously-buggy entries.
2. Verify fold/no-fold equality across the matrix and the `OBSERVABLE_INCLUDE(0) Z0` before/after `X_ERROR(0.25) 0` witness pair in both fold modes.
3. Review and enumerate every diagnostics-text change as a compatibility surface before merging.

Stage 4 (delete and split, Pass 2):

1. Delete `GaugeTracker` (`crates/stab-analysis/src/circuit_to_dem/gauge.rs`), the forward sensitivity machinery, and the now-unreachable forward fallback seam.
2. Plan the module split so no file crosses the 1200-line policy and delete the differential seam.
3. Preserve the public analyzer options surface (`approximate_disjoint_errors`, `allow_gauge_detectors`, `ignore_decomposition_failures`, `block_decomposition_from_introducing_remnant_edges`, `fold_loops`) with no removed public items lacking `docs/MIGRATING-0.2.md` notes.

### WS2b success criteria

1. Stage 0: the reverse tracker handles `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`; the fold path no longer falls back for them; and the three-family fixtures byte-match pinned Stim with `fold_loops` enabled, including inside repeat blocks.
2. `docs/plans/analyzer-consolidation-plan.md` exists and its matrix fixtures are committed before Stage 2 engine code changes (stage gate, checkable from history).
3. The Stage 2 differential test ran green across the matrix, with the known-bug exception list exactly matching the review witnesses and nothing else.
4. After Stage 3, every matrix entry byte-matches pinned Stim v1.16.0, including unpaired Pauli includes in both fold modes and gauge circuits; the executed review witness pair is a committed oracle row.
5. After Stage 4, exactly one sensitivity-propagation engine exists: `rg` finds no gate-semantics `undo_`/propagation implementations outside `sparse_rev_frame_tracker.rs`, and `gauge.rs` and the fallback seam are gone.
6. The structural quadratic is gone: an `analyze_errors` smoke benchmark on a large generated circuit (rotated surface code, distance 25, 1000 rounds) completes within the benchmark harness bound and shows at least an order-of-magnitude improvement over the recorded pre-fix measurement of the same input; the result is recorded as a diagnostic, not a promoted ratio.
7. No stab-analysis source file exceeds 1200 lines; clippy and fmt clean; `cargo test -p stab-analysis -p stab-core` passes.
8. The reopened `analyze_errors` checklist rows (decomposition, Pauli-target observables, gauge detectors) return to implemented status with their new fixture evidence listed.

## WS3: Grammar And CLI Compatibility Batch (M)

Independent of WS1/WS2; each item carries its fix, oracle fixture row, and checklist/docs update in one change set.

1. Bare `REPEAT`: reject `GateCategory::ControlFlow` in `parse_instruction` (`crates/stab-model/src/circuit/parser.rs:290`) with Stim's missing-brace diagnostic class, and make `Gate::validate`/`CircuitInstruction::new` refuse block-only gates so the impossible state is unrepresentable (`crates/stab-model/src/gate/mod.rs:672`).
2. Combiner spacing: carry pending-combiner state across whitespace-split tokens in `parse_targets` (`crates/stab-model/src/target.rs:298`) so `X0 *X1` and `X0* X1` parse as Stim does (`vendor/stim/src/stim/circuit/circuit.cc:186-192`); keep dangling-combiner errors at line ends; extend round-trip tests and the parser fuzz corpus (`just rust::parser-fuzz`).
   Note: attached forms such as `X0*X1` already tokenize correctly through the per-token `*` split; the defect is exclusively the spaced forms, so the fix must not disturb attached-token behavior.
3. `convert --in_format`: drop the `default_value = "01"` (`crates/stab-cli/src/convert.rs:22`) so the flag is required like Stim; add a CQ-CLI test asserting exit 1 and stderr class for the missing flag, and assert no output path is opened or truncated before argument validation; correct the checklist row.
4. `gen` header floats: format the four header probabilities with stab-core's Stim-compatible formatter (`crates/stab-cli/src/lib.rs:666`); oracle `gen` case with a `1e-05` and a seven-significant-digit probability.
5. Broken pipe per D2: re-raise `SIGPIPE` in `crates/stab-cli/src/bin/stab.rs` for status-141 parity; keep reporting for real I/O failures; add the qualification-plan broken-pipe test.
6. Legacy mode flags: scan the whole pre-`--` argument vector for exactly one legacy mode flag instead of first-position-only (`crates/stab-cli/src/lib.rs:392`); keep multiple-flag rejection.
7. `REPEAT(args)` headers: accept and drop parenthesized arguments to match Stim's unvalidated block gates (`vendor/stim/src/stim/circuit/circuit.cc:213-218`).
8. `E` target tolerance per D3: widen `TargetRule::PauliList` to admit combiner and inverted forms for `E`/`ELSE_CORRELATED_ERROR`.
9. `detect` deprecation ordering: emit flag-driven deprecation warnings before routing validation and preflight (`crates/stab-cli/src/detection.rs:131`), aligning with the tested warning-before-error ordering used for `--frame0`.
10. `sample_dem` flags per D4: mark both observable-routing flags hidden, document them in the checklist, and sync the help topic.
11. `FlexPauliString` double sign: parse the already-stripped dense body with a sign-rejecting variant (`crates/stab-algebra/src/pauli.rs:803-806`, root cause `pauli.rs:540`); add regressions mirroring Stim's rejections for `+-X`, `--X`, `-+X`, `i-X`, `-i+X`.

### WS3 success criteria

1. Bare `REPEAT` and `REPEAT[tag]` without a brace are rejected with the missing-brace diagnostic class, `Gate::from_name("REPEAT")` cannot construct a `CircuitInstruction`, and both are pinned by diagnostics tests plus an oracle rejection fixture.
2. `MPP X0 *X1` and `MPP X0* X1` parse, round-trip to Stim's canonical printing, and byte-match pinned Stim through a `convert --in_format=stim --out_format=stim` oracle row; attached forms keep their current behavior; dangling line-end combiners still reject; `just rust::parser-fuzz` passes.
3. `stab convert` without `--in_format` exits 1 with the same stderr class as pinned Stim, without opening or truncating any output path, as a committed CLI test; the checklist row no longer claims the old default.
4. `stab gen` output for `--after_clifford_depolarization 0.00001` and `0.123456789` byte-matches pinned Stim, as committed exact-output oracle rows.
5. `stab sample --shots 100000 ... | head -c 1` exits with status 141 and empty stderr, matching pinned Stim, as a committed test; genuine output I/O failures still report errors.
6. `stab --in <file> --sample` (mode flag not first) behaves identically to pinned Stim; multiple legacy mode flags still reject; both pinned by tests.
7. `REPEAT(0.5) 3 { ... }` parses and reprints without the arguments, byte-matching pinned Stim; `E(0.1) X0*X1` and `E(0.1) !X0` are accepted with ignored decorations, byte-matching pinned Stim through analyze_errors on a small fixture.
8. `stim detect --prepend_observables --append_observables` and the Stab equivalent both emit the deprecation warning before the combination error, with warning-before-error ordering pinned byte-exactly in JSON and human modes.
9. `stab sample_dem --help` no longer lists the observable-routing flags, the checklist names both, and the help topic matches.
10. All five doubled-sign `FlexPauliString` forms are rejected with tests mirroring Stim's rejection set, and no currently-accepted valid form regresses (existing parse tests stay green).
11. Every item's change set includes its oracle fixture row and checklist/docs update, verified by review against `oracle/fixtures/manifest.csv` diffs.

## WS4: Release Pipeline Unblock (S to M)

Complete before any v0.2.0 release attempt; the release freeze from P0.0 stays in force until Pass 1 closes.

1. Draft verification endpoint: `release_by_tag` (`ops/release/src/github.rs:408-416`) uses `GET /releases/tags/{tag}`, which per GitHub's documentation returns published releases only, so the mandatory post-upload verification in `publish_draft` (`github.rs:86`) and `verify-remote-draft` (`github.rs:130-134`) fail against a real draft.
   Re-read after creation by listing releases (paginated, bounded) and requiring exactly one entry with the expected tag and `draft == true` (list-releases is the endpoint documented to include drafts for push-access users); keep by-tag lookup for the `Published` state only.
   Mock-fidelity note: the existing HTTP-level mock publisher answers by-tag lookups for drafts, which real GitHub does not, and that fidelity gap is exactly why the defect survived the current test suite; the fix therefore includes making the mock return 404 for by-tag queries against drafts so the regression is representable.
   Rehearse once against a scratch repository and tag before release day.
2. `docs/RELEASING.md` run-identity capture: replace the `gh workflow run` stdout capture (`docs/RELEASING.md:77-80`) with dispatch followed by polling `actions/workflows/release.yml/runs?event=workflow_dispatch` filtered by head SHA, requiring exactly one match, or move capture into a read-only `stab-release` subcommand per the no-shell-logic rule.
3. Token hardening: wrap `CratesIoToken` (`ops/release/src/registry.rs:41-66`) and `GitHubToken` (`ops/release/src/github.rs:190-213`) in `secrecy::SecretString` with `expose_secret()` only at the transmitting call sites; check the latest stable `secrecy` version on crates.io before adding it.
4. Workflow hygiene: add `persist-credentials: false` to the `ci.yml` and `m12-benchmarks.yml` checkouts; optionally extend `ops/architecture/src/workflow_actions` to require it on every `actions/checkout` use; pin `cargo install just --version <x.y.z> --locked` and a `version:` input for `setup-uv`.
5. Just hygiene: add `[working-directory: '..']` to `justfiles/release.just` recipes for uniformity with every other module.

### WS4 success criteria

1. The mock GitHub server returns 404 for by-tag lookups of draft releases, the old code fails that suite, and the fixed code passes it; by-tag remains the verifier for the `Published` state with its own test.
2. A documented rehearsal against a scratch repository and throwaway tag completed `create-draft` end to end (draft created, six assets verified remotely, post-mutation verification passed) and `verify-remote-draft` passed against the live draft; the rehearsal revision and output digests are recorded in the progress report.
3. Every command sequence in `docs/RELEASING.md` executes as written against the rehearsal setup, including run-identity capture resolving to exactly one workflow run.
4. Both token types are `secrecy`-wrapped with exposure only at the transmitting call sites; the existing credential-boundary tests still pass and a test asserts no `Debug`/`Display` leak for the wrappers.
5. All workflow checkouts set `persist-credentials: false` and the architecture check fails if a future checkout omits it; CI tool installs are version-pinned; `just architecture::check` passes.

## WS5: One Owner Per Invariant (M to L, Pass 2)

The recurring duplication theme; ordered by drift risk.
No intended behavior changes; every deletion must preserve byte-identical output where a public contract exists.

1. Result-format encoding: add a fallible-reservation constructor to `stab_records::MeasureRecordWriter`, make the facade's `FallibleSampleEncoder` wrap it, and delete the forked `encode_*` functions (`crates/stab-core/src/sampling_output_compat.rs:424-517`); make `begin_result_type` a DETS-only no-op matching Stim (`crates/stab-records/src/result_formats.rs:430`, `vendor/stim/src/stim/io/measure_record_writer.cc:42-43`), keep `begin_dets_result_type` as the typed entry point, and migrate the guarded callers in `crates/stab-core/src/detection/output.rs` and `crates/stab-cli/src/convert.rs`.
2. Streaming record decode: add a per-record `RecordStreamReader` to `stab_core::advanced::records`; reduce the duplicated m2d and sample_dem r8/ptb64 readers (`crates/stab-cli/src/detection.rs:754`, `crates/stab-cli/src/sample_dem.rs:660`) to transport adapters; then convert `convert` to streaming per D5.
3. Facade error identity: re-export `stab_records::{FormatError, RecordResult}` through `stab_core::advanced::records` (or converge the tiers on one `FormatError`); expose `RecordFormat::sample_format` so the ptb64 exception has one owner (`crates/stab-records/src/sinks.rs:611`).
4. ops/bench validators: promote `Sha256Digest`/`GitCommit` (`ops/bench/src/qualification/runtime/protocol.rs:95-105`) to a shared types module, deserialize digest fields into them, and delete the roughly nineteen local validators including the uppercase-hex-accepting `is_digest` (`ops/bench/src/qualification/validation/values.rs:23`); keep one `sha256_hex`; add a `WorkerRequestSpec` builder owning worker argv, limits, and expectations for probes and rejection preflights; replace the hand-synced 28-arm group `matches!` with one static registry table.
5. stab-model gate metadata: put `produces_results`-style flags, qubit-count participation, heralded-result production, pad/metadata-only target roles, and the alias list on `GateInfo`, and derive the string-matching sites (`crates/stab-model/src/circuit/counts.rs:40-52`, `crates/stab-model/src/circuit/api.rs:145-177`, `crates/stab-model/src/gate/metadata.rs:188-205`); unify the quadruplicated tag-escaping in `model_tag.rs`/`dem/tag.rs`; this also completes the WS1 MPAD counting consolidation.
6. Smaller same-theme items: reuse `Circuit::count_*` in `circuit_detecting_regions.rs:621-706`; read tracker-owned counters in `circuit_inverse/reverse_flow.rs` instead of duplicating them; share one payload-byte estimator between `circuit_pass.rs` and `circuit_transforms.rs`; share one exact median implementation in ops/bench (`compare_evidence.rs:101` versus `statistics.rs:260`); delegate facade `Display`/equality to component types instead of copying text (`crates/stab-core/src/resources.rs:510-528`, `crates/stab-core/src/matched_error.rs:135-143`); add in-place canonicalization entry points to stab-analysis so `matched_error.rs:88-92` stops paying full clone round-trips.

### WS5 success criteria

1. Exactly one implementation of each result-format encoding exists in the workspace: `rg` finds no `encode_r8`/`encode_b8`-style logic outside stab-records, and the facade's all-format seeded equivalence tests still pass unchanged.
2. `begin_result_type` on a HITS writer is a no-op mirroring upstream, pinned by a unit test copied from the Stim writer contract, and the previous unconditional-reset behavior has a regression test proving the trap is closed.
3. m2d and sample_dem pass their existing oracle rows through the shared `RecordStreamReader`, the two CLI decoder copies are deleted, and `convert` streams with a memory-bound test (allocation-counter or equivalent) plus an over-64-MiB success fixture replacing the old cap rejection; docs and checklist updated per D5.
4. A consumer fixture in the architecture consumer check names the sink error type through stab-core paths only.
5. In ops/bench, `is_digest` is deleted, an uppercase digest in a ledger field fails validation (regression test), `registered_group_count` is derived from the registry table, and probe/rejection argv construction goes through `WorkerRequestSpec`; `just bench::qualification-check` and worker reproducibility pass.
6. Gate classification and aliases have one owner: the string-matching sites are deleted and a completeness test asserts derived views agree with the gate table for every gate.
7. All duplication deletions preserve behavior: full workspace tests pass, the full result-format corpus is byte-identical, and no checklist row changes state from this workstream alone.
8. The progress report records net implementation deletion by crate and owner, with generated fixtures and tests reported separately.

## WS6: Hostile-Input CPU Budgets (M)

Memory admission is rigorous across the workspace; close the CPU-shaped gaps the same way.
Items 1 and 7 are Pass 1 members per the execution overlay (confirmed witnesses); items 2 through 6 and 8 are backlog behind the promotion triggers.

1. `reduce_pauli_product` quadratic (`crates/stab-analysis/src/circuit_simplify.rs:600`): dedupe with a seen-set instead of scanning the `order` vector per target; reachable from `sample`/`detect` compilation via one large MPP line because parse limits deliberately do not bound targets per instruction.
2. Memoize the 24-entry single-qubit H/S BFS table in a `LazyLock` and key the seen-set on the tableau value, not `to_string()` (`crates/stab-analysis/src/circuit_simplify.rs:539-543`).
3. Flatten budget: count expanded `SHIFT_COORDS` occurrences (or add a visited-operations budget) and precompute per-block counts once per repeat block (`crates/stab-analysis/src/circuit_transforms.rs:389`, `:527`).
4. Flow-generator solver: mutate only target positions of each row's input in place instead of densifying per gate, and add an instruction budget on the no-repeat path (`crates/stab-analysis/src/circuit_flow/generators.rs:765-779`, `generators/flatten.rs:15-22`).
5. Missing-detectors elimination: track the non-invariant unsolved-row count to skip the dead scan, index rows by contained column, and compose the budget with qubit count (`crates/stab-analysis/src/circuit_missing_detectors.rs:588-614`).
6. `circuit_to_tableau`: hoist the local gate tableau out of the group loop, pass the typed `Gate` instead of re-looking up by name, and charge straight-line compositions to the existing work meter (`crates/stab-analysis/src/circuit_tableau.rs:220`).
7. `Circuit` recursive drop: mirror `crates/stab-model/src/dem/drop_impl.rs` with iterative Drop/Clone/PartialEq for `Circuit`, or enforce the nesting ceiling in `append_repeat_block` by construction; deeply nested API-built circuits currently abort the process on drop.
8. Parser preallocation: cap the circuit parser's initial capacity like `MAX_DEM_PREALLOCATED_ITEMS` (`crates/stab-model/src/circuit/parser.rs:242-267`); give `BitVec::zeros`/`resize_zeros` the same `try_reserve_exact` treatment `BitMatrix::zeros` already has (`crates/stab-bits/src/lib.rs:223-229`).

### WS6 success criteria

1. Pass 1 members: the 100k-distinct-target MPP witness compiles within an explicit time bound with the quadratic gone, and an API-built `Circuit` with nesting deep enough to overflow the old recursive drop is constructed, cloned, compared, and dropped without abort (or rejected at a constructive ceiling), each with a regression test that fails against the pre-fix code.
2. Backlog members, when promoted: each closed gap carries a witness-derived test with an explicit time or work bound, and new budget rejections use typed limit errors consistent with the existing `ResourceLimit` taxonomy, documented as deliberate divergences where Stim would attempt the work.
3. `BitVec` allocation failure returns `StorageAllocationFailed` instead of aborting, covered by a test using an absurd bit length (rides with item 8 when promoted).
4. No behavior change on valid inputs: the full oracle implemented-only run stays green.

## WS7: Engine Performance Structure (M to L, Backlog)

Required before Stim performance-ratio claims on these paths; benchmark-first per the performance-claims policy; backlog behind the promotion triggers.

1. Small-frame gate coverage: teach `SmallStabilizerFrame` single- and two-qubit tableau application, or compile `S`/`CZ`/`SWAP` and friends to native operations, so common gates stop falling off the 64-qubit fast path to the dense frame (`crates/stab-engine/src/sampling/small_frame.rs:16-32`).
2. Precompile `DirectDetectorFramePlan` into an op list at compile time and make per-shot execution allocation-free, enforced with the same allocation-counter tests the sampling session already has (`crates/stab-engine/src/detection/frame.rs:442-486`).
3. Sweep-conditioned `m2d`: add an oracle case against `stim m2d` with sweep bits on a circuit where a sweep-controlled Pauli anticommutes with a random collapse (`crates/stab-engine/src/detection/mod.rs:94-111`); implement frame-propagation semantics if divergence is confirmed, otherwise document the deliberate choice; a confirmed divergence promotes this item into the active pass as a correctness prerequisite.
4. Benchmark-gated items: add `stab_kernels_simd::pauli_right_multiply_block` only if PERFQ ratios show the scalar kernel as a bottleneck, keeping the scalar path as the differential reference; re-back `Tableau` with two `BitMatrix` planes plus sign `BitVec`s as the scheduled architecture item that also lifts the 512-qubit cap (`crates/stab-algebra/src/tableau.rs`, `crates/stab-algebra/src/limits.rs:10`).

### WS7 success criteria

1. A plan-selection test asserts circuits containing only Clifford gates plus measurements within 64 qubits (including `S`, `CZ`, `SWAP`) compile to the small-frame path, and a differential test proves small-frame and dense-frame outputs agree at fixed seeds across a gate matrix.
2. The detector-frame path has an allocation-counter test proving zero per-shot heap allocations after compile, mirroring the sampling session's existing test.
3. The sweep-`m2d` oracle case exists and either passes byte-exactly against pinned Stim or the documented-divergence entry exists in the plan and checklist with the fixture pinned to Stab's chosen semantics.
4. Any adopted benchmark-gated optimization cites its PERFQ measurement in the progress report; no optimization is merged on assertion alone.

## WS8: Harness, Test-Quality, And Polish Batch (S each, Backlog)

Backlog behind the promotion triggers; the compat-corpus and oracle stderr-class items are recommended early promotions because they strengthen Pass 1 evidence.

Oracle and CI hygiene:

1. Fixture-lane binary identity: pass an explicit target directory (or pinned `CARGO_TARGET_DIR`) to the fixture-lane build and derive the executed binary path from it, mirroring the CQ1 lane (`ops/oracle/src/main.rs:866-874`, `:410-416`).
2. Assert pinned Stim's stderr class on statistical `source=stdout` rows (`ops/oracle/src/fixtures.rs:1132-1164`).
3. Evaluate the pre-commit instruction-doc policy against the staged index instead of the worktree, and/or add the doc-policy check to CI as a backstop (`ops/pre-commit/src/instruction_docs.rs:137`).
4. Report skipped-by-exclusion staged files in the large-file scan summary and anchor exclusions to known roots (`ops/pre-commit/src/large_files.rs:19-38`).
5. Document (or fix via a temp index checkout) the staged-versus-worktree scope of the pre-commit rustfmt/clippy lane (`ops/pre-commit/src/main.rs:458-494`).
6. Bounded reader joins and kill-before-reap (pidfd or `WNOWAIT`) in both process supervisors (`ops/oracle/src/process.rs:308-317`, `:363-374`; `ops/bench/src/process.rs:292-304`).
7. Record `transport_exit_status` alongside the contract status in core-fixture receipts (`ops/oracle/src/fixtures/qualification.rs:517-526`).
8. Construct a `CheckedFixtureManifest` once per command instead of re-validating the manifest four to five times per run (`ops/oracle/src/fixtures/qualification.rs:132-149`).

Test fixes and additions:

1. Fix the double-hex-encoded compat-corpus fixtures so the four intended rejection paths execute (`test-support/compat-corpus/src/lib.rs:458`), keeping one explicit record-count-mismatch case.
2. Add one full noiseless generated-circuit golden per code family captured from pinned Stim.
3. Add one pinned-Stim SAT/WCNF byte fixture per mode for `crates/stab-analysis/src/dem/sat.rs`.
4. Delete the duplicated stab-core `Target` contract tests and the wrapper test that re-runs five registered tests (`crates/stab-core/tests/stim_format.rs:1003-1157`), keeping the CQ2-named owner.
5. Document the zero-width b8/ptb64 strictness as intentional on `read_records` and add per-command oracle checks where such input is accepted (`crates/stab-records/src/result_packed.rs`).

API and docs polish (pre-1.0 while breaking is cheap):

1. Return `bool` from infallible predicates such as `commutes`/`intersects` (`crates/stab-algebra/src/pauli.rs:418-434`).
2. Convert silent no-op invariant fallbacks to precise internal errors per the coding rules (`crates/stab-algebra/src/pauli.rs:400-405`, `crates/stab-algebra/src/iter.rs:216-219`, `crates/stab-analysis/src/circuit_inverse/reverse_flow.rs:497-507`).
3. Debug-assert the shift bound in `with_output_sign_mask` (`crates/stab-algebra/src/tableau.rs:85-102`).
4. Give `mbqc_decomposition` a typed three-way answer (decomposition, none-in-Stim, not-yet-implemented) or complete the table (`crates/stab-analysis/src/mbqc_decomposition.rs:113`).
5. Restore exact vendor flow-descriptor spellings or correct the "raw pinned" doc (`crates/stab-model/src/gate/flows.rs:3-14`).
6. Enable `missing_docs = "warn"` on stab-core first and backfill rustdoc tier by tier (226 of 269 facade-defined public items are undocumented).
7. Replace hand-rolled shifting with `Vec::insert`/`Vec::remove` in `SparseXorVec`, removing three `#[allow(clippy::indexing_slicing)]` blocks, and rename or document `from_sorted_items` canonicalization (`crates/stab-bits/src/lib.rs:650-727`).
8. Track `Tableau::random` non-uniformity in the plan so it cannot be promoted to a compatibility surface before the exact sampler exists (`crates/stab-algebra/src/tableau.rs:44-76`).
9. Revise the error taxonomy once, coordinated: ptb64 shot-count violations as format errors on record-IO paths, typed execution/storage variants instead of compile-flavored catch-alls, and consistent allocation-failure classes (`crates/stab-core/src/result_formats.rs:49-56`, `crates/stab-core/src/error.rs:168-179`, `crates/stab-core/src/detection/buffers.rs:26-39`); record in `docs/MIGRATING-0.2.md`.
10. Fix the asymmetric bootstrap percentile rounding or document the conservative direction (`ops/bench/src/qualification/runtime/statistics.rs:318-320`); share the exact median into the legacy compare surface.

### WS8 success criteria

1. With `CARGO_TARGET_DIR` pointed at a scratch directory, `just oracle::run --case smoke/tiny-circuit` provably executes the freshly built binary (stale-binary regression test or receipt assertion), and statistical rows assert Stim's stderr class (test with a mislabeled fixture failing closed).
2. `git rm --cached AGENTS.md` followed by the hook run fails the instruction-doc policy (staged-tree evaluation), covered by a hook test; the large-file summary names skipped-by-exclusion staged files.
3. Supervisor hangs are bounded: a test with a `setsid`-escaping child that holds the pipe open still returns the timeout error within the grace bound in both ops crates.
4. All four previously-masked compat-corpus rejection paths execute (mutation check: breaking each validation makes exactly one fixture fail); the noiseless gen goldens and SAT/WCNF fixtures byte-match pinned Stim.
5. `cargo doc` under `missing_docs = "warn"` is clean for stab-core; the three `indexing_slicing` allows in `SparseXorVec` are gone; the error-taxonomy revision ships with migration notes and updated pinned message tests in one change set.

## Sequencing (Dependency-Gated Batches)

1. Gate 0: P0.0 release-authorization freeze plus P0 bookkeeping, documentation only, merged before any product fix.
2. Batch A (Pass 1, parallel, gated on Gate 0): WS1; WS3 items 1 through 3; WS4 item 1.
3. Batch B (Pass 1, gated on Gate 0): WS2a; the remainder of WS3; WS4 items 2 through 5; WS6 items 1 and 7.
4. Batch C (Pass 1, gated on `docs/plans/analyzer-consolidation-plan.md` and its committed fixtures): WS2b Stages 0 through 3.
5. Batch D (Pass 2, gated on Batch C parity evidence): WS2b Stage 4; WS5.
6. Batch E (gated on Batches A through D): full oracle corpus rerun, inventory regeneration, one `--tier full` qualification rerun, un-reopening checklist rows with fresh evidence, and restoring `docs/plans/GOAL.md` to an active release state.
7. Backlog items enter a batch only through a promotion trigger and never block a gate.

## Verification

- Per change set: `cargo fmt --check --all`, `cargo clippy --workspace --all-targets`, targeted `cargo test -p <crate>`, and the relevant `just oracle::run --milestone Mx`.
- Per batch gate: `cargo test --workspace`, `just oracle::run --implemented-only`, `just oracle::record --check-clean`, inventory regeneration checks, and the stable-component commands from `CONTRIBUTE.md`.
- End of plan: full CQ tier rerun and benchmark qualification rerun from a clean committed revision before promoting any reopened row.

### Statistical Test Contract

Every statistical test added by this plan must declare: a fixed seed panel or a deterministic seed-generation rule; the sample count; the tested joint distribution or invariant (not only marginals where the defect class is correlational); and the acceptance interval with its false-positive budget, consistent with the oracle suite's existing familywise budget discipline.

## Plan Done Criteria

1. Every workstream's success criteria hold from one clean committed revision, recorded with command output digests in `docs/plans/post-review-remediation-progress-report.md`.
2. Every reopened checklist row is restored with named fresh evidence, `docs/qualification-status.md` regenerates with no reopened rows attributed to this review, and `docs/plans/GOAL.md` is restored to an active release state in the same change set as the last evidence.
3. `cargo test --workspace`, the stable-component test commands, `just oracle::run --implemented-only`, `just oracle::record --check-clean`, `just oracle::matrix --check`, `just qualification::correctness-check`, and `just bench::qualification-check` all pass.
4. A CQ `--tier full` run and the affected benchmark qualification groups rerun clean from the final revision, and their digests are recorded.
5. No source file newly exceeds 1200 lines (no-regression invariant, not a refactoring mandate), clippy and fmt are clean, and no new `#[allow]` of the panic-family lints exists in non-test code.
6. The review's eleven P1 findings each map to a named regression test or committed oracle fixture that fails against the pre-fix code.
7. The progress report's final entry records net implementation deletion by crate for Pass 2 and the complete backlog with each item's promotion triggers.

## Risks

- WS2b is the only item with real architectural risk: it replaces an engine that passes a large existing suite, so byte-exact equivalence fixtures must exist before the old engine is deleted (Stage 1 gate), the three fallback families must be ported first (Stage 0 gate), and diagnostics-text changes must be reviewed as a compatibility surface (Stage 3 gate).
- The WS3 grammar changes touch the public `.stim` contract; every one needs an oracle fixture in the same change set, and the parser fuzz corpus should run before merge.
- The D1 decision freezes a platform-flavored byte baseline; if a future x86-64 qualification host is added, `.dem` byte-exact oracle rows must be scoped to the frozen baseline platform or the decision revisited.
- Evidence churn: batching inventory regeneration reduces cost but means the dashboard shows reopened surfaces for the duration of the plan; that is intentional honesty, not drift.
- WS2b Stage 2's differential exception list is the guard against silently blessing new bugs: any differential mismatch not on the known-witness list stops the stage until explained.
- The release freeze depends on `docs/plans/GOAL.md` being honored as the single release-authorization contract; if any release action is attempted during the freeze, the P0.0 prohibition list is the controlling document.
