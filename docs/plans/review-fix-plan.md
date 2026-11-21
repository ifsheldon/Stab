# Review Fix Plan

This plan covers the findings of the code review run on 2026-11-19 against commit `c7e3a679`.
Its scheduled steps are complete; the outcome table below records what each one did.
Each finding is marked **verified** when the main session reproduced it or traced it in the code, and **reported** when only a reviewer did.
A reported finding is reproduced first, and dropped from the plan with a note if it does not hold.
Timings quoted here are shared-host diagnostics and promote no performance claim.

## Outcome

Every scheduled step is finished as of 2026-11-20 and Phase D as of 2026-11-21; the open items at the end of Phase D and the areas listed under "Not Reviewed" remain.

| Step | Outcome |
| --- | --- |
| A1 | Done. Inlining rejects detectors and observables a collapse randomizes; the tracker's flag became `AnticommutationPolicy`. |
| A2 | Done. A pipe replays when every record is retained; otherwise the new `replay-input-not-rewindable` diagnostic. |
| A3 | Done. The same-file guard applies to regular files only, for input and output pairs as well as output pairs. |
| A4 | Done. Ledger family, regression test, and pinned reproduction; both programs refuse 64 or more components. |
| A5, A6 | Done in the ledger. |
| B1 | Done. On the review's one-million-wide case under a 16 MiB budget, peak memory fell from 39.4 MB to 18.1 MB. |
| B2, B3, B4 | Done. |
| B5 | Done. `ReferenceSample` and `SweepRecord` replace `Vec<bool>` and `&[bool]`. |
| B6 | Done. A `CliffordString` is charged exactly its six planes. |
| B7 | Done for the passes that can exceed a millisecond; the three left unpolled are listed in the architecture README. |
| B8 | Done. A 300-million-bit fold went from about 0.55 s unpolled to negligible. |
| B9 | Done for `analyze_errors`, and for `gen` once Phase D made it stream its text through a buffer. |
| C1 | Done. Analysis also stopped refusing circuits with more than `u64::MAX` ticks, matching Stim. |
| C2 | Done with a lazily resolved flag instead of a batch clear; 200,000-qubit reset 92.8 ms to 1.4 ms, and the flow generators 2.35 s to 0.38 s at 4,000 qubits. |
| C3 | Done. Replay of a wide model fell from 76 MB to 26 MB peak. |
| C4 | Dropped. Rounding the window to a power of two was 6% faster on one case and 4% slower on another. |
| C5, C6 | Done. Compiling one million two-term products fell from 101.7 ms to 61 ms. |
| C7 | Done. About 12% on a measurement-heavy case and 20% on an `ELSE_CORRELATED_ERROR`-heavy one; the local-decomposition part gained about 2%. |
| C8 | Done. Printing 20,000 long tags fell from 36.6 ms to 17.5 ms for circuits and 27.7 ms to 8.5 ms for models. |
| C9 | Done. `DemTarget` is eight bytes and the shift offset is no longer a target; the quadratic-output case needs about 735 MiB instead of 1,468 MiB and now runs at the default budget. |

Three items found during the work were added to Phase D, and two of their premises turned out wrong when measured there: the measurement-to-detection compile is not superlinear, and the flow generators were already quadratic.

## Priority Rule

`AGENTS.md` ranks the project's goals in three tiers, and a higher tier wins.

1. Good, well-designed, and correct APIs; performance comparable to or better than Stim; and a well-designed modular architecture.
2. Stim compatibility.
3. Backward compatibility.

A step therefore chooses the design that is right even when it changes a public signature, and a deliberate difference from Stim is recorded in the parity ledger as a design choice.
Every public change still gets its `docs/MIGRATING-0.2.md` note, because `AGENTS.md` requires documentation to move with the API.

## Decisions Needed Before Work Starts

Each decision has a recommended answer, and the plan below assumes it.

1. **`m2d --ran_without_feedback` on non-deterministic detectors.** Recommended: reject, in the library function as well as the CLI, with no lenient mode. Good design and Stim agree here: a detector that anticommutes with a reset has no defined value, so a converted circuit that keeps it is wrong.
2. **Piped `--replay_err_in`.** Recommended: accept a non-seekable replay input whenever the whole replay fits in the retained store, and keep the current refusal only when records were declined. The alternative is to keep refusing every pipe and record a divergence.
3. **Device files in the same-file guard.** Recommended: exempt paths that are not regular files (character devices such as `/dev/null`, FIFOs), which is the rule the standard-stream guard already follows.
4. **`--fold_loops` when the period search does not fit the budget.** Recommended: fall back to unrolling, as Stim does when its hare fails.
5. **Library reference-sample APIs that return `Vec<bool>`.** Recommended: change them to return a packed, typed reference sample. The earlier draft kept the signatures to avoid a break; under the priority rule that reason no longer holds, and a byte per measurement is the wrong shape for a value every consumer immediately packs.
6. **PTB64 trailing partial group in `convert`.** Recommended: document only. Every streaming conversion leaves partial output when the input turns out malformed late, so a precheck for this one case would be inconsistent and impossible on a pipe.
7. **`DemTarget` redesign.** Recommended: schedule it (step C9) instead of deferring it. The earlier draft deferred it because it is a public `stab-model` type.

## Settled: Overflowing `--shots` And `--seed`

Stab's usage error is the right behaviour and stays.
Pinned Stim also rejects these values: `9223372036854775808`, `18446744073709551615`, and `99999999999999999999999` all exit 1 with "Got non-int64 value".
Only exactly `18446744073709551616` slips through Stim's overflow check and is read as zero, so `--seed 2^64` samples with seed 0 and `--shots 2^64` prints nothing with exit 0.
That is a Stim defect, not a contract, and rejecting every unrepresentable value is recorded as a deliberate design choice.
This is done: the `cli.argument-values` ledger family and the generated `docs/stim-parity.md` row now cover it, and its test `tests::args::empty_and_missing_flag_values_are_rejected` includes the `2^64` cases for `--shots` and `--seed`.

## Phase A: Stim Compatibility

One commit per step unless noted.
Every step that changes CLI behaviour updates the parity ledger source under `oracle/stim-v1.16-parity/` and regenerates `docs/stim-parity.md` with `just oracle::parity-render`.

### A1. Reject non-deterministic detectors in `m2d --ran_without_feedback` (verified)

- **Now:** `circuit_with_inlined_feedback_with_limits` builds `SparseReverseFrameTracker::new(..., false)` at `crates/stab-analysis/src/circuit_feedback.rs:79`, so anticommutation never fails. Stim's `WithoutFeedbackHelper` uses the failing default.
- **Repro:** `R 0`, `H 0`, `M 0`, `DETECTOR rec[-1]` with `m2d --ran_without_feedback`. Stim exits 1; Stab exits 0 and prints records.
- **Change:** pass `true`, so `circuit_with_inlined_feedback` and `circuit_with_inlined_feedback_with_limits` reject such circuits for every caller, and check that the tracker's anticommutation error surfaces as a typed circuit-validation error (exit 1 on the CLI), not as an internal error.
- **No lenient mode:** the function has one correct behaviour, so no option is added. If the literal `bool` on `SparseReverseFrameTracker::new` has no remaining caller that passes `false`, remove the parameter; if it has, replace it with a two-variant enum so call sites say what they mean.
- **Tests:** an oracle fixture row for the detector case and for `OBSERVABLE_INCLUDE(0) Y0` after `M 0`, comparing exit status and stderr class against pinned Stim; a deterministic-feedback circuit that must still convert.
- **Docs:** `docs/MIGRATING-0.2.md` for the stricter library behaviour.

### A2. Accept piped replay input when nothing needs a second pass (verified)

- **Now:** `validate_replay_input` in `crates/stab-cli/src/sample_dem.rs` always calls `input.seek_to(offset)`, so `--replay_err_in /dev/stdin` fails with "Illegal seek" even when the store kept every record.
- **Change:** have `retain_or_skip_all` report whether any record was declined. Seek only in that case. When nothing was declined, delivery reads the store alone and never touches the input again.
- **Unchanged:** a non-seekable input whose replay does not fit the budget still fails before any output exists, with a message that names the cause and suggests a larger `--memory-budget` or a regular file, instead of the bare OS error.
- **Tests:** piped replay for `01`, `b8`, `r8`, HITS, and DETS matches the file-backed output byte for byte; a piped replay under a budget too small to retain it fails with no output file created.
- **Docs:** the two-pass replay paragraph in `docs/architecture/README.md` (currently says the seek happens unconditionally) and the function's doc comment.

### A3. Let two outputs name the same device (verified)

- **Now:** `crates/stab-cli/src/io_plan.rs:416` treats any two existing paths with the same identity as an alias, so `--out /dev/null --obs_out /dev/null` is refused. Stim accepts it.
- **Change:** apply the path guard only when the shared target is a regular file. The guard exists to stop one output truncating another file role; a device or FIFO cannot be truncated that way.
- **Keep:** symlinks to a regular file, hardlinks, and normalized paths are still refused. The file type is taken from the opened handle, not from a second path lookup, so there is no window between the check and the open.
- **Tests:** `/dev/null` for both outputs succeeds on each command that has two output roles; the four existing identity rows still pass.
- **Docs:** the contract text of the `resource-safety.cli-file-*-identities` ledger rows says "every active file-role pairing"; narrow it to regular files.

### A4. Ledger the 32-term decomposition divergence (verified)

- **Now:** Stim's `brute_force_decomp_helper` shifts a plain `int`, so a component with 32 or more terms loses terms (`D33` is dropped on a 34-term case). Stab shifts a `u64` and is correct. Nothing records this.
- **Change:** no code change. Add a `stim-bug` divergence family next to `analysis.circuit-to-dem-error-decomposition`, with the pinned reproduction, and a Stab regression test on a component of at least 33 terms asserting that the printed components XOR back to the original error.
- **Check:** what Stab does at 64 or more terms, where its own `u64` mask would run out. It must be a typed error or a wider path, not a wrong answer or a panic.

### A5. Document the PTB64 partial-output behaviour (reported)

- Reproduce a 70-record `01` to PTB64 conversion leaving a 56-byte file and exit 1.
- Extend the existing PTB64 divergence row's rationale to say that complete groups are written before a trailing partial group is refused.

### A6. Ledger the `2^64` argument divergence (done)

- Done as described under "Settled" above; it is uncommitted and goes in with the A4 and A5 ledger commit.

## Phase B: Memory Budget And Cancellation

These are hostile-input policy fixes: admission before allocation, fallible allocation, and bounded unpolled work.
Each adds a row or sentence to `docs/architecture/resource-policy-inventory.md`, and a `docs/MIGRATING-0.2.md` note where a previously accepted budget is now refused.

### B1. Charge `m2d`'s input record batches (reported, with a measured repro)

- **Now:** the measurement reader in `crates/stab-cli/src/detection.rs` grows to `M2D_BATCH_SHOTS` (256) records and the sweep reader does the same; neither is admitted. The reviewer measured a 39 MB peak under `--memory-budget 16MiB` on a one-million-wide measurement.
- **Change:** before `io.activate()`, admit `RecordStreamStorage::new(format, width, batch).bytes()` for the measurement reader and the sweep reader, the way `ConvertStorage::admit` does. When the full 256-record batch does not fit, cap the batch growth at the largest power of two that does, so a wide circuit under a small budget still runs with smaller batches instead of being refused outright.
- **Why capping is safe:** `m2d` output does not depend on batch size.
- **Tests:** the wide-measurement case under a small budget stays under it (assert on the admitted bytes, not RSS); output is byte-identical across batch caps.

### B2. Stop the bounded reader doubling past its cap (verified)

- **Change:** `input.try_reserve(grown)` becomes `try_reserve_exact(grown)` at `crates/stab-cli/src/input.rs:168`. The geometric growth is already in `grown`.
- **Test:** a reader that reports its largest requested capacity, fed one byte more than the cap.

### B3. Give the reverse tracker the analyzer's held bytes (verified)

- **Now:** `sync_error_budget_base` in `circuit_to_dem/reverse_fold.rs:104` sets the accumulator's base only. The tracker's base stays zero during circuit-to-DEM, so it can grow against the whole budget on top of the output already held; the overrun is caught only at the end of the instruction. `circuit_detecting_regions.rs` already sets the tracker's base correctly.
- **Change:** also call `self.tracker.set_base_bytes(output_bytes + errors.admitted_bytes())` in the same function.
- **Test:** the reviewer's `budget2.stim` shape, asserting the refusal reports a need no larger than the budget plus one growth step.
- **Risk:** some circuits near the limit that completed by luck will now be refused earlier. That is the intended contract; note it in the migration guide.

### B4. Fall back to unrolling when the fold search does not fit (verified)

- **Now:** `search_shifted_recurrence` returns a budget error when three tracker copies do not fit, so `--fold_loops` refuses a circuit that the unfolded run completes under the same budget.
- **Change:** a memory-budget refusal raised by the search's own checks or by a probe step returns `ShiftedRecurrenceSearch::Exhausted`, after dropping both clones. Cancellation and every other error still propagate. A genuine shortage then resurfaces from the unrolled run with its real numbers.
- **Test:** the `foldbudget.stim` shape succeeds with `--fold_loops` and produces the same DEM as without it.

### B5. Return a packed reference sample from the library (reported)

- **Now:** `try_reference_sample`, `circuit_reference_sample`, and `reference_measurement_record_with_sweep_into` in `stab-engine`'s `sampling` module build a `Vec<bool>`, one byte per measurement, with no budget check, while admission charges only the packed bits; the sweep path builds it twice.
- **Change:** make the packed record the public value. A `ReferenceSample` type owns the packed words and the measurement count, offers `len`, `get(index)`, an iterator of bits, and a borrowed word view, and is what the plan already caches, so `try_reference_sample` returns a shared handle to the cached value instead of copying it.
- **Sweep input:** take the sweep record as packed bits too, through a typed constructor that checks the width, so the function no longer accepts a `&[bool]` of the wrong length.
- **Callers:** update `m2d`'s reference path, the reference-sample tree, and tests; remove `to_bools` and the `Vec<bool>` buffer helpers if nothing else uses them.
- **Result:** the admitted bytes are the allocated bytes, and the under-charge disappears instead of being papered over.
- **Check first:** whether an existing type in `stab-bits` or `stab-records` already fits, so no second packed-bits type is introduced.

### B6. Build `CliffordString` concat and repeat without transient copies (reported)

- **Where:** `crates/stab-algebra/src/clifford.rs:551-604`, with `from_table_indices`, `table_indices`, and `BitVec::from_bits`.
- **Change:** write the six result planes directly with word-level copies and shifts into storage reserved with `try_zeroed_words`, so the admitted planes are the only allocation and every allocation is fallible. This also removes the per-gate decode and per-bit rebuild.
- **Tests:** existing concat and repeat tests plus lengths that are not multiples of 64 on either operand, and a repeat whose pattern length is 1, 63, 64, and 65.

### B7. Poll the remaining full-width detection passes (reported)

- **Where:** `detection/api.rs` (`try_zero_word_planes`, `copy_batch_into_frame_planes`, `sweep_planes.fill`), `detection/mod.rs` (reference XOR, `xor_words`), `detection/streaming.rs` and `conversion_window.rs` (observable plane resize, fill, XOR).
- **Change:** route them through the existing `fill_polled` and `poll.for_chunks` helpers with the session's `WorkPoll`, mapping cancellation as `ensure_lanes` does.
- **Scope limit:** start by measuring which of these exceed about a millisecond at a realistic worst case. Passes that cannot will be listed in the architecture README's unpolled-stretches sentence instead of being changed.

### B8. Poll and speed up folded reference repeats (reported, with a measured repro)

- **Where:** `append_repeated` in `sampling/reference_bits.rs`.
- **Change:** double the pattern in place until it is at least 64 bits, then copy whole words at a fixed shift, charging the `WorkPoll` per chunk of words.
- **Tests:** pattern lengths 1, 3, 63, 64, 65, and 130 at start offsets that are and are not word-aligned, compared with the current bitwise result; a cancellation test.

### B9. Charge named output buffers in `analyze_errors` and `gen` (reported)

- Route both through `OutputStorage::file_buffer(named)`, matching `convert`, `sample`, `detect`, and `m2d`.
- Streaming `gen`'s circuit text instead of building a `String` is not included; the reviewer saw no overrun, and it goes to Phase D.

## Phase C: Contained Speed And Cleanup

Each item is measured before and after, pinned with `taskset -c 7`, and must leave seeded output byte-identical.
An item whose measured gain is within noise is dropped, not committed.

| Step | Change | Where | Status |
|---|---|---|---|
| C1 | Remove the unread `ticks_left` counter and its `count_ticks()` walk. | `circuit_to_dem/reverse_fold.rs:60,95,256` | verified |
| C2 | Stop `PauliString::set` rescanning the whole string when a term is cleared: add a batch clear that rescans once, and use it in `undo_reset` and the flow generators. | `stab-algebra/src/pauli.rs:373`, `stab-analysis/src/circuit_pauli.rs:635` | verified |
| C3 | Size replay sessions at one word per plane instead of 1,024 shots. | `dem_sampling/session/replay_session.rs:59` | reported, RSS measured |
| C4 | Round the DEM sample capacity down to a power of two no larger than 1,024 so windows do not straddle RNG blocks. | `dem_sampling/session/batch.rs:144` | reported |
| C5 | Reuse one scratch buffer for MPP product terms during lowering. | `frame/program/lower/build.rs:259` | reported |
| C6 | Store `PauliTerm` qubits as `u32`. | `frame/program/arena.rs:58` | reported |
| C7 | Remove per-error and per-instruction allocations in analysis: fixed arrays in local decomposition, borrowed instructions on the `ELSE_CORRELATED_ERROR` stack, reverse iteration without collecting. | `reverse_fold/local_decomposition.rs:38`, `reverse_fold.rs:194`, `sparse_rev_frame_tracker/pauli_product.rs:9` | reported |
| C8 | Write unescaped tag runs in one call; validate recovered tag bytes once. | `stab-model/src/model_tag.rs:140`, `dem/tag.rs:45` | reported |

### C9. Redesign `DemTarget` as a packed target (reported, refusal measured)

- **Now:** `DemTarget` in `crates/stab-model/src/dem.rs:713` is a 16-byte enum with `RelativeDetector`, `LogicalObservable`, `Separator`, and `Numeric(u64)`. Stim's target is 8 bytes. The size doubles the reverse tracker's sets, the error accumulator's arena, and DEM output, and one quadratic-output model was refused at the default 1 GiB budget where Stim finished in 757 MB.
- **Design fault beyond size:** `Numeric` exists only to carry the offset of `shift_detectors`, which is not a target at all. It lets an error or a detector instruction hold a value that is meaningless there.
- **Change:** make `DemTarget` an 8-byte newtype over a `u64` with typed constructors and accessors (`detector()`, `observable()`, `is_separator()`, and a matchable `kind()` view), and move the `shift_detectors` offset into that instruction's own typed field. Detector ids are already capped at `2^62 - 1` and observables at `u32`, so the packing loses nothing.
- **Order:** an `Ord` that keeps today's ordering (detectors, then observables, then separator) so DEM output and merge order stay byte-identical; this is asserted by the existing oracle rows.
- **Scope:** `stab-model` first, then `stab-analysis`, `stab-engine`'s DEM sampling, and the CLI, in one change set because the type crosses all of them.
- **Measure:** peak admitted bytes and wall time of `analyze_errors` on the surface-code and quadratic-output cases before and after, and review `ARENA_SLACK_FACTOR` once the element size is halved.
- **Placement:** last in this phase, after C1 and C7 have settled the analysis code it touches.

C3 and C4 change how many shots a batch holds.
Seeded output is documented as independent of capacity, and both steps re-run the seeded-determinism tests and the `sample_dem` two-sample oracle rows to confirm it.

## Phase D: Larger Changes

These were not scheduled by the plan above and were done afterwards as one batch.
Each performance item was reproduced and measured first, and dropped with its numbers when the premise did not hold.

| Item | Outcome |
| --- | --- |
| Superlinear `MeasurementToDetectionCompiler::compile` on `MPP`-heavy circuits | Dropped; the premise was wrong. The time is the reference sample, which `sample --shots 1` pays as well, and it grows linearly: 1.48 s at 120,000 products and 13.18 s at one million. With `--skip_reference_sample` the one-million-product circuit takes 0.05 s. The comparison against detection compilation misled because detection sampling needs no reference sample. |
| Cubic `circuit_flow_generators` for wide resets | The premise was wrong; growth was already about quadratic, which is the floor because the output has about `N²` bits. A per-pivot memmove, a second scan, and scans of untouched columns were removed: a 4,000-qubit reset went from 0.536 s to 0.229 s with identical generators. |
| Analysis error taxonomy | Done. `AnalysisError` names what failed in typed variants, the CLI codes follow them, and message text is unchanged across a 7,666-run comparison. The migration guide lists the mapping. |
| Matchable `DemInstruction` view | Done. `DemInstruction::view()`, and each kind now stores exactly its own data, so an item is 88 bytes instead of 112. |
| Pack `CircuitReferenceSigns` | Done. |
| Share the periodic word fill | Done as `stab_bits::fill_periodic_words`; the `CliffordString` repeat cases went from about 38 ms to about 23 ms. |
| Public API review | Done as a review of every crate, about 90 findings. The fixes that remove a real hazard were applied; the rest are listed below. |
| Fold search with the analyzer as tortoise | Done as a port of Stim's `run_loop`. Successful output was byte-identical on 1,128 runs; a wide-tracker case fell from 31.7 MB to 23.7 MB peak. A failed search now falls back to unrolling, so the reported error is the unrolled run's, as in Stim. |
| `ConversionPlan` detector terms in one arena | Done. One million two-term detectors fell from 140.0 MB to 117.0 MB. The arena first cost about 3% in `m2d`; hoisting the arena lookup and simplifying the term index left `m2d` about 9% faster than before the change. |
| One tree walk for the circuit and DEM models | Done for clone, drop, equality, admitted bytes, and tag restore, with one opaque-tag record. The walks that compute different things per dialect stayed separate. |
| File splits | Done: `record_stream.rs`, `reverse_fold.rs`, `circuit_flow/generators.rs`, both parsers' diagnostics, and four other files near the limit. |
| Unitary-to-tableau rewrite | Done. Ten qubits went from 67.3 s to 0.023 s, where pinned Stim takes 0.169 s. Acceptance follows Stim's snapping, and Stab keeps a column-norm check that Stim lacks, recorded in the ledger. |
| Short-length path in the XOR kernel | Dropped. The kernel and a scalar loop differed by under 0.6 ns per call in both directions, and a short path was slower at eight words. |
| Stream `gen` output | Done. Rotated distance 301 fell from 180 MB to 112 MB peak, the admission bound was re-derived as one mebibyte plus 768 bytes per physical qubit, and a named output's buffer is charged. |
| Release tooling | Done. The publish wait requires the sparse index as well as the API, and a test fails if release-time Cargo could discover the repository's Cargo configuration. |

Three Stim divergences were found and fixed along the way: the sparse-form rule for flow text, a `PAULI_CHANNEL_2` with one non-zero component, and a target-less `PAULI_CHANNEL_2` with two.

### Open After Phase D

[Open after Phase D: details and recommendations](open-after-phase-d.md) expands every item below.

Decisions that change the crate graph or a parity surface, each needing its own answer:

- **One memory-budget type.** Done on 2026-11-21 as the leaf crate `stab-foundation`, which also holds cancellation, the estimate, the byte span, and the diagnostic severity.
- **One representation of a refused budget.** Done on 2026-11-21: `MemoryBudgetExceeded` in `stab-foundation`, one `memory_budget_exceeded()` accessor on every error type, and no message-text matching in the CLI.
- **The observable-id domain.** Done on 2026-11-21: one `ObservableId` with the 32-bit domain of the DEM format, checked where a circuit is parsed or built. A 64-bit domain was considered and rejected because pinned Stim's DEM format refuses ids above `0xFFFFFFFF`.
- **Symlinks and unsafe path components on outputs.** Decided on 2026-11-21: no command rejects them, as in Stim, and the architecture README records the path trust boundary that makes this the policy instead of a gap.

Larger redesigns the API review proposed and this batch did not do:

- Done on 2026-11-21: one flow check returns a `FlowVerdict` per flow, and a refused budget, a cancellation, or an invalid circuit is an error instead of `false`.
- Done on 2026-11-21: "no logical error found" is a `LogicalErrorSearch::NotFound` outcome, and each search takes one named options value.
- Done on 2026-11-21: the gate-data getters return `Option`, and `GateDataUnavailable` is removed.
- Done on 2026-11-21: one function per operation taking an options value in analysis, and one session constructor per plan taking an options value in the engine.
- Done on 2026-11-21: the engine's run progress, status, summary, and error types are written once.
- Done on 2026-11-21: `CircuitInstruction::gate_arguments()` and `gate_targets()` are typed, infallible views, which removed about 120 impossible branches in analysis and the engine.
- Done on 2026-11-21: `ErrorAnalyzerOptions` uses nested option types instead of five independent booleans.
- Budget or remove the `Vec<Vec<bool>>` record codec family, which only CLI tests use outside `stab-records`.
- Done on 2026-11-21: gate flow and decomposition metadata are typed tables in `stab-model`, checked against the vendored Stim gate data, and no product crate parses gate text at run time.
- Done on 2026-11-21: `stab_model::advanced` went from 57 exports to 27, with dead items deleted and free wrappers turned into methods.

Smaller items noticed and left:

- Stab is 2.6 to 4.8 times slower than Stim on `--fold_loops` circuits with a long transient, in the tracker's set operations.
- `ConversionOperation` is 40 bytes because of its repeat fields; a side table could save about 16 MB per million detectors.
- `circuit_reference_signs` computes a byte per bit before packing.
- A public `Tableau::to_unitary` needs a more efficient construction than the test helper.
- Done on 2026-11-21: the fixed caps in `stab-model` that reported an invalid detector error model are typed fixed limits.

## Order And Delegation

1. A1, A2, A3 in sequence: all three touch `stab-cli` and its tests.
2. A4, A5, A6 together as one ledger commit: documentation and tests only.
3. B2, B3, B4 next: small, verified, and independent.
4. B1 and B9 together: both are CLI output and input admission.
5. B5, B6, B8, B7 one at a time: each is in a different module, B7 last because it starts with a measurement.
6. Phase C in table order, with C9 last and on its own.

Implementation steps go to one subagent at a time per crate so that no two writers share files, and the main session reviews each diff, edits `docs/`, runs the checks, and commits.
Nothing is pushed until asked.

## Verification

- Per step: the targeted `cargo test -p <crate>` for the touched crate, and a direct comparison against pinned Stim for every Phase A step.
- Per phase, before the last commit of the phase: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace --locked`, `just oracle::parity-check`, `just oracle::parity-render --check`, `just oracle::parity-run`, `just oracle::run --all-rows`, `just oracle::result-formats --check`, `just architecture::check`, `just architecture::docs-check`, `just bench::e2e-check`, and `just docs::api-check`.
- Stack safety: every step that touches lowering or a recursive walk re-runs the 64 KiB stack tests, and the phase is confirmed on the x86-64 CI runner after the push, since this host cannot fail those tests.

## Not Reviewed

The review did not read `ops/` outside part of `ops/release`, the detecting-regions, missing-detector, and DEM-search parts of `stab-analysis`, or `circuit/parser.rs` beyond black-box fuzzing.
A second pass over those areas is a separate task.
