# Engine Speed And Architecture Plan

Status: Active since 2026-11-14, after the fearless SIMD plan completed. E1, E2, E3, E5, and E6 are complete; E4 is in progress. This plan follows a November architecture and performance review and is separate from the [Stim core parity plan](stim-core-parity-and-lean-evidence-plan.md) and the [fearless SIMD plan](fearless-simd-stable-rust-plan.md).

## Summary

Close the user-scale speed gaps that the current E2E suite cannot see, remove the work limits that reject ordinary QEC workloads, and collapse the duplicated execution machinery that causes both.

The hot frame loops are already competitive with Stim. The gaps are in what surrounds them: a slow general reference simulator, missing frame kernels for most Clifford gates, admission limits measured in worst-case work, allocation-heavy error analysis, and codec logic repeated across the engine, records, and CLI.

## Review Evidence

Measured on 2026-11-13 on the AArch64 development host against pinned Stim v1.16.0, best of five runs on one pinned core, with a release build that includes `portable-simd`. These are shared-host diagnostics. They promote no performance claim and are not E2E evidence.

| Workload | Stim | Stab | Stab/Stim |
| --- | ---: | ---: | ---: |
| `sample`, rotated memory X, d=21, 1 shot | 14.9 ms | 792.9 ms | 53x |
| `sample`, d=7, 1000 rounds, 1 shot | 10.0 ms | 273.1 ms | 27x |
| `sample`, d=21, 200,000 shots, `--skip_reference_sample` | 1183 ms | 922 ms | 0.78x |
| 1000 qubits, 200 layers of `CX` and `H`, 100,000 shots | 522 ms | 347 ms | 0.66x |
| The same circuit with `CZ` and `S` | 465 ms | 7506 ms | 16x |
| `analyze_errors --decompose_errors`, d=21 | 728 ms | 984 ms | 1.35x |
| `analyze_errors --fold_loops --decompose_errors`, d=21 | 153 ms | 217 ms | 1.42x |
| `detect`, d=7, 1000 rounds, 100,000 shots | 3019 ms | 3474 ms | 1.15x |
| `detect`, d=25, 1000 rounds, 1000 shots | 696 ms | 369 ms | 0.53x |
| `sample_dem`, d=11 model, 200,000 shots | runs | refused | n/a |
| `sample`, d=25, 1000 rounds, 1 shot | runs | refused | n/a |

Hardware profiling is unavailable on the host, so attribution uses gdb stack sampling with 40 to 125 samples per workload.

Limit probes, run on 2026-11-14 with one shot unless noted:

| Workload | Stim | Stab | Limit hit |
| --- | --- | --- | --- |
| `detect`, 14 MB circuit with 1,500,000 lines | 0.18 s | refused | 1,000,000 source lines |
| `sample_dem`, 34 MB model with 1,500,000 lines | 0.39 s | refused | 1,000,000 source lines |
| `detect`, 147 MB flattened circuit | 0.88 s | refused | 64 MiB input file |
| `detect`, d=35, 1000 rounds, 1,224,000 detectors | 0.06 s | refused | 1,000,000-bit record width |
| `detect`, repetition code, 200,000 rounds, 64 shots | 0.02 s | refused | 1,000,000 expanded instructions |
| `sample`, repetition code, 200,000 rounds | 0.02 s | refused | 1,000,000 operations per shot |
| `sample_dem`, d=11 model, 2253 shots | runs | refused | 64,000,000 sampled error applications |
| `gen`, repetition code, distance 200,001 | 0.00 s | refused | generator size cap |

The product crates define 132 limit-like constants, and `stab_algebra::Tableau` is capped at 512 qubits although a tableau of that size occupies about 131 KB.

Verified directly during the review:

- The reference sample falls from `sampling/reference_tableau.rs` to the generator-list `StabilizerFrame` whenever a circuit exceeds 100,000 expanded operations or uses anything other than `H`, `CX`, and Z-basis reset and measurement.
- At review time `sample_dem` rejected any run whose shots multiplied by error count exceeded 64,000,000. Commit `7e5a7f28` has since made that bound per shot, so the two `sample_dem` shot-count refusals in the tables below no longer reproduce.
- `sample` rejects more than 1,000,000 expanded per-target operations per shot, while `detect` counts instructions and accepts the same circuit.
- `format_stim_float` in `crates/stab-model/src/circuit/printing.rs` formats each non-integer DEM argument twice at 34 significant digits.
- Writing 8192 `ptb64` shots through `--out` issues 184,448 eight-byte `write` calls.
- `ErrorMatcher` is exported from `stab-analysis`, has no consumer in any other crate, and belongs to the ledger family `analysis.error-explanations`, whose status is `deferred`.

Reported by review agents and not independently re-run: the 323 ms to 7.7 ms reference-sample result with only the operation cap removed, the per-gate and lane-width ratios, the `DirectZ` comparison, the codec timings, and the unused-API scan of `stab-records`. Each milestone re-measures what it relies on before acting.

## Principles

- One production owner per capability: one reference simulator, one resolved circuit program, one codec owner.
- Deletions are part of the work, not a later cleanup. A milestone is not done while the path it replaces still compiles.
- Limits bound memory, recursion, and representation; CPU time belongs to the caller and is controlled by cancellation, not by caps.
- No gate, threshold, or semantic workload is relaxed, and no Stim ratio is promoted from diagnostics.
- Seeded outputs may change where an implementation changes random-stream consumption; statistical and semantic equivalence remain required and exact Stim streams remain out of scope.

## Milestone E1: General Reference Simulator

### Tasks

1. Implement one inverse-tableau stabilizer simulator in the engine that supports every gate the frame program supports, with deterministic measurements answered from the tracked inverse and random measurements collapsed in place.
2. Drive it from the resolved frame program instead of `SampleProgram`, and fold repeat blocks by state recurrence so long memory experiments do not pay per-round cost.
3. Cache the reference sample on the compiled plan instead of recomputing it for every session.
4. Route `sample`, the `m2d` reference in `sampling/reference.rs`, and `ReferenceSampleTree` through it.
5. Delete `CssReferenceTableau`, the 100,000-operation cap, and the reference role of `StabilizerFrame`.

### Tests

- Differential tests against the existing general simulator for every deterministic measurement across the gate catalog, run before the old path is deleted and retained afterward against stored expectations.
- Property tests that every reference sample satisfies the circuit's deterministic detectors, including folded and nested repeat blocks.
- Pinned-Stim differential for `sample` on circuits whose measurements are all deterministic.

### Benchmarks

- A reference-dominated workflow belongs in the E2E suite within the existing case cap: an X-basis memory `sample` whose single-shot cost is visible. The suite's `sample-surface.d25-b8` case is one.
- Target: that case, and single-shot `sample` at d=21, within the suite's `1.25x` Stim ceiling.

### Done Criteria

- One reference simulator exists and no operation-count threshold selects between implementations.
- Session creation performs no reference computation when the plan already holds one.

### Checkpoint (2026-11-14)

E1 landed at `b06f4e00`. One word-packed inverse-tableau simulator replaced `CssReferenceTableau` and the reference role of `StabilizerFrame`, the reference is cached on the plan, and repeat folding compares stabilizer states so memory rounds fold.

Deviation: the simulator still consumes the `SampleProgram` operation stream, and re-driving it from the unified program moves to E3. `StabilizerFrame` remains for general-frame sampling and as the differential test baseline until E3.

Seeded `sample` output is byte-identical to the previous implementation on 26 comparisons covering surface, color, repetition, and random Clifford circuits up to 2000 qubits.

Single-shot `sample`, best of three interleaved runs on one pinned core, in milliseconds:

| Circuit | Before | After | Stim |
| --- | ---: | ---: | ---: |
| Rotated memory X, d=11 | 25.7 | 3.9 | 3.6 |
| Rotated memory X, d=21 | 770.0 | 14.1 | 14.3 |
| Rotated memory X, d=25 | 1798.6 | 22.9 | 24.8 |
| d=7, 1000 rounds | 272.2 | 8.9 | 9.4 |
| Random Clifford, 1200 qubits | 2305.5 | 323.3 | 311.1 |
| Random loop, 400 qubits, 300 iterations | 4828.3 | 146.6 | 218.4 |
| Random loop, 2000 qubits, 50 iterations | 24,101.7 | 651.8 | 2190.2 |

The suite's `sample-surface.d25-b8` case moved from `6.96x` to `1.09x` Stim in the smoke tier.

Open items: generic Clifford gates in the reference cost about five times Stim's on a `CZ` and `S` layer circuit (72 ms against about 15 ms), loops with a period above one are not folded, and admission estimates still reflect the old simulator's storage so the limits milestone must revisit them.

## Milestone E2: Complete Frame Kernels

### Tasks

1. Lower every Clifford gate in the catalog to dedicated word-parallel frame operations at compile time, as `H` and `CX` are today.
2. Drop Pauli gates from the frame program at compile time, since they do not change a Pauli frame.
3. Normalize `MPP` products into a compile-time term arena so execution allocates nothing per batch.
4. Re-measure the `DirectZ` plan against the Pauli-frame plan and delete `DirectZ` if the frame path is at least as fast on its own workloads.
5. Remove the generic per-bit tableau path and the unreachable instruction arms it leaves behind in `detection/frame.rs`.

### Tests

- The existing gate semantic contract runs unchanged against the new kernels.
- Statistical two-sample differentials against pinned Stim for a circuit that exercises every two-qubit Clifford.
- Allocation tests prove zero per-batch allocation for `MPP`, `SPP`, and every gate family.

### Benchmarks

- An E2E case with a non-`CX` gate mix replaces or joins an existing sampling case within the cap.
- Target: the `CZ` and `S` layer circuit within `1.25x` of Stim.

### Done Criteria

- No gate executes through a per-bit path during frame sampling or detection.

### Checkpoint (2026-11-14)

E2 landed at `28689edf`. Every unitary catalog gate lowers to a word-parallel frame operation derived at compile time from its pinned `H`, `S`, `CX` decomposition; identity-action gates compile to nothing; MPP and SPP products use a compile-time term arena; the per-bit tableau path is gone.

Seeded `sample`, `detect`, and `m2d` output is byte-identical to the previous build on 39 comparisons.

Best of three interleaved runs on one pinned core, in milliseconds:

| Workload | Before | After | Stim |
| --- | ---: | ---: | ---: |
| `CZ` and `S` layers, 200,000 shots, reference skipped | 12,647.9 | 1563.8 | 913.6 |
| `CZ` and `S` layers, 1 shot, reference skipped | 1074.9 | 1057.6 | 15.0 |
| `CX` and `H` layers, 20,000 shots | 104.4 | 92.8 | 119.2 |
| `detect`, color code d=9, 200,000 shots | 82.1 | 69.5 | 58.9 |
| `detect`, d=11, 350,000 shots | 332.4 | 314.6 | 286.7 |
| `detect`, d=7, 1000 rounds, 100,000 shots | 3484.8 | 3293.5 | 2962.9 |

The per-shot cost of the `CZ` and `S` circuit is now below Stim's, about 2.5 against 4.5 microseconds.
The remaining gap on that circuit is a fixed compile cost of about one second: `SampleProgram` compilation builds a gate tableau and a heap transform for every non-`CX` gate target, about 300,000 times.
That cost belongs to E3, which retires `SampleProgram`, so E3 now carries a speed target as well as the simplification.

`DirectZ` was measured and not deleted: at 1,000,000 shots it takes 27 to 28 ms against 24 to 27 ms for the Pauli-frame plan, so it is no faster than the path it bypasses.

## Milestone E3: One Resolved Circuit Program

### Tasks

1. Make the frame program the single resolved circuit representation and retire `SampleProgram`, giving `count_determined_measurements` a home on the E1 simulator.
2. Confirm with exhaustive plan-kind probes that general-frame sampling is unreachable, then delete `SessionFrame::General`, `sample_general_into`, `sampling/noise.rs`, the sample-mode arms of `sampling/execute.rs`, and `StabilizerFrame`.
3. Finish lowering: resolve gate identity and arguments at compile time so execution never matches on gate names or re-parses arguments per batch.
4. Replace the four repeat-tape interpreters and four circuit walkers with one tape and one cursor.
5. Compile `m2d` references without building a full sampling plan.

### Tests

- Plan and fingerprint tests are updated in the same change, with the plan fingerprint schema revised if the executable identity changes.
- Folded and unrolled equivalence properties cover the unified tape.

### Benchmarks

- No E2E case may regress beyond the seeded `1.15x` rule; this milestone claims simplification, not speed.

### Done Criteria

- The engine has one circuit program type, one repeat interpreter, and one circuit walker.
- `sample` and `detect` derive admission from the same counts.

### Checkpoint (2026-11-14)

E3 landed. `CircuitProgram` is the only compiled form; the Pauli frame, the reference simulator, the determined-measurement count, sweep correction, and detection conversion walk it through one cursor; and the `m2d` reference compiles without a sampling plan. `SampleProgram` and `DirectZ` are deleted.

Seeded `sample`, `detect`, and `m2d` output is byte-identical to the previous build on 39 comparisons. It changes only for a circuit of exactly one non-reset measurement with a nonzero flip or preceding single-qubit Pauli channel, the shape `DirectZ` used to take.

Best of three interleaved runs on one pinned core, in milliseconds:

| Workload | Before | After | Stim |
| --- | ---: | ---: | ---: |
| `CZ` and `S` layers, 1 shot, reference skipped | 1054.7 | 20.7 | 14.9 |
| `CZ` and `S` layers, 20,000 shots | 1102.8 | 69.2 | 106.7 |
| `CX` and `H` layers, 20,000 shots | 91.3 | 67.0 | 118.9 |
| Random Clifford, 1200 qubits, 2000 shots | 323.6 | 98.6 | 302.1 |
| `sample`, d=11, 300,000 shots | 201.2 | 199.9 | 251.8 |
| `detect`, d=11, 350,000 shots | 300.7 | 301.5 | 285.1 |

Deviations and consequences:

- General-frame sampling is reachable for programs above the 256 MiB plan budget, so `GeneralFrame`, `sample_general_into`, `sampling/noise.rs`, and `StabilizerFrame` remain as that fallback over the unified program. E4 decides whether a byte budget replaces the fallback.
- The conversion executor and the sampling-estimate walker stay separate because their semantics differ from program execution.
- `sample` counts expanded instructions as `detect` does, so a d=25 circuit with 1000 rounds is admitted by both. Limit values are unchanged.
- `count_determined_measurements` with unknown input undercounted after full resets and is corrected.
- Without `DirectZ`, one measurement on a very high qubit index is refused on storage limits, as `detect` already did.
- The executable-contract schema moved from 5 to 6.

## Milestone E4: Limit Policy Reset

The project owner requested this policy on 2026-11-14 after a review of every limit in the product crates. It replaces the limit policy recorded in the [A2 resource policy inventory](../architecture/a2-resource-policy-inventory.md).

### Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion. It does not mean bounded CPU time, which belongs to the caller.

| Kind of limit | Decision |
| --- | --- |
| Format and representation invariants, such as target values, detector ids, repeat counts, and number tokens | Keep unchanged. |
| Repeat nesting | Keep one shared 256-level bound, and bring the 128-level detecting-region bound in line. |
| Byte budgets for sessions, batches, compiled plans, and materialized output | Keep as the single admission dimension per operation, make them caller-selectable everywhere, raise the defaults, and expose one CLI option. |
| Count caps beside a byte budget, such as source lines, declarations, retained targets, record width, and flattened operations | Remove; the byte budget already bounds what they proxy. |
| CPU-work caps, such as sampled error applications, replay work, operations per shot, expanded instructions, aggregate repeat iterations, and analysis work units | Remove as defaults. |
| CLI input-file caps | Remove the fixed 64 MiB cap; library callers keep `ParseLimits` for their own admission. |
| Search and SAT budgets | Keep as explicit caller policy with a budget-exhausted result, and review the defaults. |
| Caps that bound a slow algorithm, such as tableau, solver, and random-tableau qubits | Remove by fixing the algorithm, leaving only a byte budget. |

### Tasks

1. Reclassify every limit constant in the product crates against the policy table and record the result in a rewritten resource policy inventory, so no constant is left unclassified.
2. Remove the count and CPU-work caps listed above from the model parser, the engine, analysis, and the CLI, keeping overflow-checked counting so that an unrepresentable size is a typed error instead of a wrapped value.
3. Give each materializing or executing operation one byte budget computed before allocation, with fallible reservation after admission as today.
4. Add one CLI option for the memory budget and document it as a Stab extension in the parity ledger.
5. Add cooperative cancellation: a caller-supplied signal polled inside repeat interpretation and between batches, so a compact hostile input cannot hold an uninterruptible unit of work.
6. Replace the dense tableau, stabilizer solver, and random-tableau implementations that are capped at 512, 512, and 64 qubits with word-parallel versions bounded only by bytes; E1 needs the tableau at surface-code scale.
7. Update [ADR 0004](../architecture/adr-0004-diagnostics-and-resources.md), the hostile-input section of the root `AGENTS.md`, the affected ledger divergence records, the CLI diagnostics, and `docs/MIGRATING-0.2.md` for removed public limit types.

### Tests

- Byte-budget boundary tests admit the exact limit and reject the first excess before any allocation, RNG advance, or sink mutation.
- Hostile-input tests keep their memory, recursion, and overflow cases; tests that only assert a removed count or work cap are deleted with the cap.
- Cancellation tests prove that a signal stops a single-shot run of a circuit with an astronomically large repeat count promptly, and that a sink error stops a run within one batch.
- CLI tests cover every workload in the limit-probe table under [Review Evidence](#review-evidence).
- Tableau and solver differential tests run at more than 1000 qubits.

### Benchmarks

- The suite's `sample-dem.surface-d11-side-outputs` case covers a realistic shot count and runs since `7e5a7f28`.
- No benchmark workload is shrunk to fit a limit.

### Done Criteria

- No default limit rejects a workload that Stim completes within the byte budget.
- Every remaining limit is an invariant, the nesting bound, a byte budget, or an explicit search budget.
- A hostile compact input can be cancelled, and cannot exhaust memory, overflow, recurse without bound, or panic.

## Milestone E5: Faster Error Analysis

### Tasks

1. Replace `BTreeSet<DemTarget>` sensitivities with one sorted small-vector XOR set that merges into reused scratch.
2. Accumulate errors through a single map lookup per error, with target lists interned instead of allocated per key.
3. Format each DEM float once and reuse the digits for both notations, and memoize by bit pattern because analyses emit few distinct probabilities.
4. Apply generic two-qubit Cliffords in the tracker through static per-gate tables instead of rebuilding a `Tableau` from strings per instruction.
5. Consume the reversed model during un-reversal and stream DEM text to the output instead of materializing it.

### Tests

- Byte-identical `analyze_errors` output against pinned Stim on the existing fixtures and on d=11, d=21, and d=25 circuits, folded and unrolled, with and without `--decompose_errors`.
- The DEM float printer keeps its pinned-Stim differential, including values just below powers of ten.

### Benchmarks

- Target: every `analyze-folded` case and an added unrolled case within `1.25x` of Stim, with peak RSS no higher than today.

### Done Criteria

- The `analyze_errors` profile is no longer dominated by allocator and float-formatting frames.

### Checkpoint (2026-11-14)

E5 landed. Tracker sensitivities are sorted XOR sets, errors accumulate through one hashed lookup against an interned arena, each DEM float is formatted once, Clifford actions come from per-gate tables, un-reversal works in place, and the CLI streams the model.

Output is byte-identical to the previous build on 88 circuit and flag combinations, including stderr and exit status.

Best of three interleaved runs on one pinned core, in milliseconds:

| Workload | Before | After | Stim |
| --- | ---: | ---: | ---: |
| `--decompose_errors`, d=15 | 290.6 | 141.3 | 247.0 |
| `--decompose_errors`, d=21 | 852.2 | 400.6 | 711.9 |
| `--decompose_errors`, d=25 | 1509.2 | 734.0 | 1229.0 |
| `--fold_loops --decompose_errors`, d=21 | 190.3 | 80.0 | 151.0 |
| Unrolled, d=7, 1000 rounds | 2515.8 | 1204.7 | 2826.2 |
| `--decompose_errors`, d=15, 150 rounds | 3103.1 | 1528.6 | 2689.9 |

Peak memory for the d=25 case fell from 190 MB to 108 MB against Stim's 121 MB.

Open item: `PAULI_CHANNEL_1` probabilities differ from Stim in the last printed digit in some analyses; this predates E5 and is unchanged by it.

## Milestone E6: Records Owns Codecs

### Tasks

1. Make record readers yield packed words and record writers consume packed words, with sparse formats iterating set bits.
2. Provide one Records-owned encoder that drains into any `io::Write`, and delete the encoders in `stab-cli` and the unused sink encoder.
3. Fold the `m2d` and `sample_dem` replay readers in the CLI into reader options on the Records stream reader.
4. Buffer every CLI output, including `--out` files, with explicit flush so write errors still surface.
5. Delete the `stab-records` visitors and record types that have no product caller, after a doctest-aware usage check.
6. Take measurement, detector, and observable counts in `convert` from the model instead of compiling an engine plan.

### Tests

- The byte-exact result-format corpus and the live pinned-Stim differential pass unchanged.
- Writer-failure and flush-failure tests cover buffered stdout and buffered files.
- Diagnostic text for malformed input stays byte-identical where the ledger records it.

### Benchmarks

- The `convert-*` and `m2d-packed-sweep` cases must not regress; no new case is required.

### Done Criteria

- The CLI contains no format-specific encode or decode loop.
- No output path issues one system call per record.

### Checkpoint (2026-11-14)

The output half of E6 landed at `588328e9`: one `RecordEncoder` in `stab-records` serves every CLI output route in all six formats, and standard output and output files are buffered with explicit flushes.

Output bytes are identical to the previous build on 53 command and format combinations.

Best of three interleaved runs on one pinned core, in milliseconds:

| Workload | Before | After | Stim |
| --- | ---: | ---: | ---: |
| `sample_dem`, d=11, sampled errors as `hits` | 6385.2 | 651.4 | 1444.8 |
| `detect`, d=11, 300,000 shots, `hits` | 1258.8 | 346.9 | 825.7 |
| `detect`, d=11, 99,968 shots, `ptb64` | 891.4 | 92.9 | 83.8 |
| `sample`, d=11, 100,000 shots, `01` | 685.9 | 76.5 | 636.8 |
| `convert`, `b8` to `hits`, 200,000 records | 2128.5 | 555.1 | 6161.8 |

Writing 8192 `ptb64` shots to a file now issues 26 writes instead of 184,448.

Behavior change: with buffered output a write failure is detected later, so the committed-shot count in the diagnostic counts shots accepted by the writer and not shots on the device. Exit status and error class are unchanged.

After E1, E2, and this half of E6, every one of the 25 suite cases is under the `1.25x` ceiling in the smoke tier.

The input half of E6 then landed: `RecordStreamReader` yields packed batches for all six formats and absorbs the CLI's `m2d` and replay readers, input is buffered without per-read zero fill, record storage is allocated fallibly after the first frame is proven, and the unused visitors and record types are deleted.

Output of `convert` across all 36 format pairs, `m2d`, and replay is identical to the previous build on 60 comparisons, including malformed input, except that a huge declared width is now a typed error instead of a process abort.

Two tasks were deliberately not completed as written.
`convert` takes its counts from the model but still compiles the engine plan once, because that compilation is what rejects detectors without measurements and applies the record-width limit.
Replay input is still parsed twice, because decoding once while producing no partial output on malformed input would hold every record in memory.

Open items: `hits` and `dets` input remain far slower than `b8` because the shared integer grammar dominates, and replay from sparse `hits` is about seven times slower than Stim because the engine walks every error for every shot instead of only the recorded hits.

## Milestone E7: Wider Detection Frames

### Tasks

1. Move `DetectorFrameState` from 64-lane `u64` planes to 256-lane `BitBlock` planes, matching circuit sampling.
2. Replace the three 64-shot re-chunkers with one multi-word plane view in `stab-records`, recorded as a revision to [ADR 0003](../architecture/adr-0003-typed-batch-families.md).
3. Measure 1024-lane rows driven by slice kernels, and adopt them only if the E2E cases improve.

### Tests

- Detection statistical differentials and the conversion-plan properties run at every lane width that ships.

### Benchmarks

- Target: `detect` at d=7 with 1000 rounds within the Stim ceiling, with no regression in `detect-observables`.

### Done Criteria

- Sampling and detection share one lane width and one batch-delivery path.

## Milestone E8: Deletions And Shared Vocabulary

### Tasks

1. Delete `ErrorMatcher`, `matched_error`, their tests, and their exports, and record the removal in `docs/MIGRATING-0.2.md`.
2. Add a gate identity enum to `stab-model` with one semantics table, and convert the per-file name matches in analysis and engine to exhaustive matches on it.
3. Replace the duplicated repeat-budget walkers in `stab-analysis` with one folded circuit summary in `stab-model`.
4. Either extract the handful of gate helpers the engine uses from `stab-analysis` or correct the architecture document's description of that dependency, with an ADR 0007 update if the graph changes.

### Tests

- `just architecture::check`, `just oracle::gates`, and `just oracle::parity-check` pass.
- Exhaustive matches replace default arms, so a new gate fails compilation instead of silently falling through.

### Done Criteria

- No deferred product surface is exported.
- Gate behavior has one table.

## Order And Dependencies

- E1 and E2 are independent of each other and deliver the largest measured gains.
- E3 depends on E1, which removes the last production user of `SampleProgram` and `StabilizerFrame`.
- E4 is independent of the engine milestones except for two links: E1 needs E4 task 6 for a tableau beyond 512 qubits, and the engine cap removals are simpler after E3 leaves one program to count.
- E5, E6, and E8 are independent of the engine milestones.
- E7 depends on E6 for the shared plane view.

Every milestone changes product code, so each invalidates controlled evidence under the release rules. Milestones should be grouped ahead of a planned evidence run instead of each requesting its own.

The closing step of this plan is the formal evidence that P10 could not produce: one controlled AArch64 full-and-soak pair in which all 25 cases pass correctness, memory, and the `1.25x` Stim ceiling, followed by baseline seeding, exact-revision CI, and `just bench::e2e-release-check`.

## Sequencing With Other Plans

- P10 phases 0 through 5 finished at `a3260617` on 2026-11-14. The E2E suite now has 11 families and 25 user-scale cases with no accepted baselines, and P10's formal evidence phase is blocked on gaps this plan owns.
- P10 changed one limit: commit `7e5a7f28` bounds DEM sampling work per shot instead of per request, so `sample_dem` no longer limits shot count. The 64,000,000 per-shot value and every other limit are unchanged, so E4 stays open with that one row already improved.
- P10 left the reference-sample path unchanged and added differential tests for it, which E1 keeps.
- Five suite cases are expected to fail the `1.25x` Stim ceiling until this plan lands. `sample-surface.d25-b8` at about `7.6x` belongs to E1, `detect-observables.color-d9-b8` at about `1.4x` to E2, `analyze-decompose.d25` at about `1.3x` to E5, and `sample-dem.surface-d11-side-outputs` at about `5x` and `detect-observables.d11-sparse-hits` at about `1.5x` to the HITS encoder in E6. These ratios are the P10 session's pinned best-of-three diagnostics.
- P10 also reported two record-reader findings that E6 must fix: readers allocate the declared width before reading, so a huge width aborts instead of returning an error, and the stream reader zero-fills 64 KiB per read. The first must be fixed before E4 removes the record-width cap.
- The fearless SIMD plan runs first. Its S1 through S4, including the move to Stable Rust 1.100, complete before any milestone here starts, so every E milestone is written, tested, and measured on the 1.100 toolchain with the `fearless_simd` kernels always on.
- The SIMD kernels are on no engine hot path today, so the port is not expected to change the gaps measured above; they are re-measured on 1.100 before E1 starts. E7 task 3 is the first place slice kernels could matter.
- Baselines seeded by that plan's S5 give the E milestones a valid `1.15x` self-regression reference on the new toolchain.

## Risks

- The diagnostics come from one shared host; the controlled host may rank the gaps differently.
- A general tableau simulator has quadratic cost per random measurement, so circuits dominated by random measurements need their own measurement before the old path is deleted.
- E3 and E6 remove public API from unpublished 0.2 crates, which is acceptable now and costly after release.
- E4 removes limits that were introduced deliberately, so a path that relied on a count cap to bound an allocation must gain a byte check before the cap goes; each removal needs that audit.
- Without work caps, a library caller that ignores cancellation can be held by a hostile compact input for as long as the work takes, which is the same exposure Stim has.
- Another change stream is active in tests and benchmarks, so milestones should avoid `tests/` restructuring beyond their own owners.
