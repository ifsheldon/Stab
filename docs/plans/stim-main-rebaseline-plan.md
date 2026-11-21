# Plan: Move The Stim Baseline To Current `main`

Written 2026-11-21.
The four decisions below were made on 2026-11-21; the work itself is not started.

## Goal

Move Stab's frozen compatibility baseline from Stim v1.16.0 (`e2fc1eca7fd21684d433aa5f10f4504ea4860d07`) to the head of `quantumlib/Stim` `main`, which is `ffe0d043598ed5df0851a3e6b4920359a23fafbd` as fetched on 2026-11-21.
In the same pass, stop reproducing Stim bugs that upstream has fixed, implement what upstream added, and report to Stim the defects Stab found that upstream still has.

## What Changed Upstream

`main` is 27 commits past v1.16.0 and reports its version as `1.17.dev0`; there is no v1.17 tag.

| Class | Count | Commits |
| --- | --- | --- |
| Core bug fixes | 3 | #1131 (16 fixes), #1123, #1111 |
| Core features and performance | 3 | #1110, #1117, #1090 |
| Build, CI, and release | 6 | #1100, #1127, #1128, #1072, #1070, #1077 |
| stimflow | 9 | #1071, #1074, #1079, #1081, #1082, #1083, #1092, #1093, #1094 |
| stimcirq | 3 | #982, #1120, #1124 |
| Dependency bumps | 3 | #1095, #1129, #1130 |

The C++ core change is small: 14 non-test files and about 35 changed lines outside the Python bindings.
The gate table, the `.stim`, `.dem`, and result-format documents, and the command-line reference are unchanged, so no public file format or CLI flag moves.
29 files under `src/stim` and `doc` changed in total, and 46 of the 163 parity-ledger families cite at least one of them, mostly through a test file.

## Decisions

All four were decided on 2026-11-21.

### 1. The baseline is pinned by commit now

`main` is unreleased and keeps moving, and upstream issue #1115 asks for a release.
The baseline is "Stim `main` at `ffe0d04` (1.17.dev)", pinned by commit.
It is re-pinned to the v1.17.0 tag when that exists, which should then be a small diff.
Waiting for the release was rejected because it keeps Stab on a baseline with a known bug it copies (see workstream C).

### 2. Ledger paths carry no version

The baseline version is in file names (`oracle/stim-v1.16-parity.toml`, `oracle/stim-v1.16-parity/`), a build directory (`target/oracle/stim-v1.16.0`), and prose in about 90 files.
The ledger becomes `oracle/stim-parity.toml` and `oracle/stim-parity/`, the oracle build directory is named for the commit by `ops/oracle`, and the version and commit are stated in one place inside the ledger and one place in `ops/oracle`, so the next move is a constant change and not a rename.

### 3. One upstream issue per bug, drafted for review

Reports are posted from a GitHub account and are public, so nothing is posted until each draft has been reviewed.
Each bug gets its own issue with a Python and a command-line reproduction against upstream `main`, the expected and actual behavior, the source line, and a suggested fix where it is a few lines.
Fix pull requests are not part of this plan; they need Google's contributor agreement and can follow an issue if upstream wants one.
Each draft says the bug was found while writing an independent implementation, and that wording is part of the review.

### 4. Stab keeps its deliberate divergences

Where the new baseline still has a bug, Stab keeps its divergence, with a ledger entry, a rationale, and a test.

## Workstreams

### A. Rebaseline mechanics

- Move the `vendor/stim` submodule pointer to the new commit.
- Rename the ledger files and directory as decision 2 says, and update `LEDGER_PATH` in `ops/oracle/src/parity.rs`.
- Change `STIM_TAG`, `STIM_COMMIT`, and the build directories in `ops/oracle/src/main.rs`. The oracle verifies the checkout with `git describe --tags --exact-match`, which fails on an untagged commit, so the check must become commit-only or accept "no tag".
- Change `[stim]` in `benchmarks/suite.toml` (see workstream H before touching it).
- Rebuild the oracle binary and its helpers, and remove `target/oracle/stim-v1.16.0` once nothing reads it.
- Check that CI builds the new Stim; #1100 adds a missing header, which helps on newer toolchains.
- Update the instruction files: the root `AGENTS.md` says "Target Stim v1.16.0 as the frozen compatibility baseline until the plan is explicitly changed", and this plan is that change.

### B. Regenerate evidence and explain every difference

- Rerun `just oracle::run --all-rows` (210 rows; 161 stored expected outputs under `oracle/fixtures/expected`), `just oracle::result-formats --check` (83 cases), `just oracle::gates`, the ignored `pinned_stim` probes in `ops/oracle`, and `just oracle::parity-run` against the new binary.
- The expected result is no difference outside the areas of workstreams C and D.
- Every difference is attributed to an upstream commit before a stored expectation is regenerated. A difference with no upstream cause is a finding, not something to regenerate away.

### C. Upstream fixes, mapped to Stab

| Upstream fix | Stab today | Action |
| --- | --- | --- |
| #1131: Newton step typo in `try_disjoint_to_independent_xyz_errors_approx` | **Copies the bug on purpose** (`dc = ab_i - ac` in `circuit_to_dem/error_decomp.rs`); a test pins that a `PAULI_CHANNEL_1` circuit is rejected | Use the correct step, flip the test, compare with the new Stim, note the behavior change in the migration guide |
| #1123: 32-bit shift drops decomposition components from the 32nd on | Already correct; ledger `stim-bug` divergence | Turn the entry into ordinary parity and compare byte for byte |
| #1131: time reversal with duplicate reset and measure-reset targets | Already diverges; ledger `stim-bug` | Compare with the fixed Stim; turn into parity if equal, otherwise explain the remaining difference |
| #1131: feedback inlining of `XCZ` and `YCZ` record controls | Stab extension | Turn into parity and compare |
| #1111: tag parsing at unexpected end of input (v1.16.0 never returns on `H[`) | Rejects with "unterminated tag"; no ledger entry | Add an oracle row now that Stim terminates, and compare the error class |
| #1131: circuit and DEM tag data on `clear`, on appending `REPEAT` blocks, and on DEM slicing | Not checked | Port the new upstream tests where Stab has the operation |
| #1131: `CircuitErrorLocation::is_simpler_than` compared an object with itself | Not checked; this is error-explanation ordering | Check whether Stab's error matching has an equivalent ordering; `explain_errors` itself is deferred |
| #1131: `sample` and `detect` closed standard input | Not applicable | Confirm no output difference |
| #1131: `throw new` typos in the sparse reverse tracker | Not applicable; Stab returns typed errors | None |
| #1131: `FlipSimulator.broadcast_pauli_errors` (two fixes), stimflow, sinter, stimcirq, the 3D match-graph drawer, the clang CI job | Out of scope | None |

The #1131 commit message lists 16 fixes; the audit reads its diff, not only the message, and reads the diff of every other commit that touches `src/stim`.

### D. New upstream behavior

- **#1110, noisy measurements in QEC inversion.** A measurement with past dependence and no future dependence now becomes a reset even when it has a noise argument, followed by `X_ERROR` or `Z_ERROR` with that argument. Implement in `circuit_inverse_qec`, port the upstream tests, and add or extend a ledger family. This changes `circuit_inverse_qec` and `circuit_time_reversed_for_flows` output for such circuits.
- **#1117, `transposed` option on `CompiledDetectorSampler.sample`, and #1090, faster `Circuit.append`.** Both are Python-binding work. Record them as deferred with the Python bindings.
- **Upstream tests.** Read every changed `*.test.cc` (`circuit`, `detector_error_model`, `circuit_inverse_qec`, `transform_without_feedback`, `error_analyzer`) and port the cases Stab's owner tests do not already cover.

### E. Re-verify the whole ledger

- All 61 divergence entries are re-run against the new binary, not only the 46 families that cite a changed file, because a rationale that says "pinned Stim does X" must be true of the new pin.
- Expected changes: three or four entries become parity (workstream C), and the wording of the rest is checked.
- `stim-bug` entries need their pinned reproductions refreshed against the new binary.
- Regenerate `docs/stim-parity.md`.

### F. Report Stab's findings to Stim

Still present on `main`, with no issue or PR found by keyword search on 2026-11-21:

1. Circuit equality ignores repeat-block tags.
2. Circuit equality compares the leftover storage of a removed repeat block (Python `pop`).
3. `convert` advertises PTB64 output and PTB64 observable output but rejects both.
4. Weighted WCNF prints `SIZE_MAX` as a literal for zero-probability errors.
5. Feedback inlining treats a sweep bit as a qubit in a mixed record and sweep `CZ`.
6. QEC inversion accepts `rec[-0]` and synthesizes extra measurements.
7. Time reversal silently drops `obs[k]` terms of a supplied flow.
8. `analyze_errors` adds a negative record offset without a bounds check on feedback lookbacks.
9. `OBSERVABLE_INCLUDE(4294967295)` prints as `L2147483647`, and `sample_dem` crashes on `L4294967295`.
10. The integer argument `18446744073709551616` is read as 0.
11. A write error other than a closed pipe, such as a full device, exits 0.
12. The hypergraph search at maxima (2, 2) drops errors that contain a `^` separator.

For each: reduce to a minimal reproduction, confirm it on a build of the new pin (workstream A comes first for that reason), write expected and actual behavior, and point at the source line.
Items 10 and 11, and the argument-parsing behaviors Stab treats as usage errors, may be intended upstream; the drafts say so instead of calling them bugs.
Before filing, search again by symptom, since a keyword search can miss a differently worded report.

### G. Open upstream issues in areas Stab implements

These are open on Stim and have not been tested against Stab:

- #1114: `solve_flow_measurements` returns no answer for a solvable flow when the batch also holds unsolvable ones.
- #1109: `flow_generators` returns flows that `has_flow` rejects for `MR 0 0` and `MPAD 1 0`.
- #764: `time_reversed_for_flows` raises on a circuit with feedback.
- #1098: whether a remnant edge introduced during decomposition carries stray logical observables.

For each, run the reproduction against Stab. If Stab has the bug, fix it as a deliberate divergence with a ledger entry; if it does not, add the case as a test.

### H. Performance evidence

- `benchmarks/suite.toml` names the comparator's version and commit, and `benchmarks/current-aarch64-evidence.toml` names evidence measured against v1.16.0.
- No upstream commit touches a C++ sampling or analysis hot path, so ratios are not expected to move, but that is an expectation and not a measurement.
- Changing the comparator invalidates the current evidence as a statement about the baseline. The formal full and soak runs are repeated on the controlled AArch64 host before any ratio is restated, following `benchmarks/AGENTS.md`.
- Until then the performance documents say which Stim commit the existing evidence was measured against.

### I. Documentation

- Root `AGENTS.md`, `README.md`, `README.crates.md`, `CONTRIBUTE.md`, `docs/README.md`, the architecture records, and comments and rustdoc in about 60 crate files that say "v1.16.0" or "pinned Stim".
- `docs/MIGRATING-0.2.md` gains a section for the user-visible behavior changes: `analyze_errors` accepts circuits it rejected (workstream C), and QEC inversion output changes for noisy measurements (workstream D).
- `docs/plans/GOAL.md` and the parity plan state the new baseline.

### J. Make the next move cheap

Add an `ops/oracle` report, for example `just oracle::baseline-diff --from <commit> --to <commit>`, that lists the upstream files changed between two commits and the ledger families whose `stim_refs` cite them.
It produced the "46 of 163" figure above by hand; as a tool it turns the next rebaseline into a checklist.
Upstream has been landing batches of fixes from automated analysis, so more moves like this one are likely.

## Things Not In The Request

- The target is an unreleased commit, so the baseline has a name and a re-pin step (decision 1).
- Every ledger divergence has to be re-verified, not only the ones upstream fixed (workstream E).
- Stored expected outputs and the result-format corpus have to be regenerated with every difference explained (workstream B).
- The benchmark comparator changes with the pin, which makes the current evidence stale as a baseline statement (workstream H).
- Upstream's new and changed tests are a source of cases to port (workstream D).
- Open upstream issues in Stab's areas may be bugs Stab shares (workstream G).
- The oracle's tag check, the CI build, and the old oracle build directory need attention (workstream A).
- The baseline version is baked into file names; removing it now avoids repeating the rename (decision 2).
- Python-only upstream changes should be recorded as deferred so the ledger stays complete (workstream D).
- stimflow, stimcirq, sinter, Crumble, and the diagram code stay out of scope, as today.
- A tool for the next rebaseline (workstream J).

## Order

1. Workstream A, including the ledger rename of decision 2, on a commit of its own, with the checks expected to fail where workstreams C and D apply.
2. Workstream B, to learn the real set of differences.
3. Workstreams C and D, each difference closing as it is fixed.
4. Workstream E, then G.
5. Workstream F drafts, reviewed before anything is posted.
6. Workstreams I and J.
7. Workstream H when the controlled host is available.

Steps 1 to 4 should land close together, because between A and C the tree compares against a baseline it does not yet match.

## Done Means

- `vendor/stim`, `ops/oracle`, and `benchmarks/suite.toml` name the new commit, and no file names the old one as the baseline.
- `cargo test --workspace`, every `just oracle::*` check, `just architecture::*`, `just docs::api-check`, and `just bench::e2e-check` pass against the new binary.
- Every ledger divergence has been re-run, and every entry upstream fixed is ordinary parity or has a stated remaining difference.
- Stab no longer reproduces a bug that the baseline has fixed.
- The upstream reports are drafted, reviewed, and either filed or explicitly held back.
- Performance documents either carry new evidence or say which commit the existing evidence was measured against.

## Risks

- **`main` moves.** Pinning a commit fixes the target, but the v1.17.0 release may differ; the re-pin is a second, smaller pass.
- **A fix upstream may differ from Stab's.** Where Stab diverged first, the fixed Stim may print something different from Stab; each such case needs a decision between matching Stim and keeping Stab's form.
- **Evidence regeneration can hide regressions.** The rule in workstream B, no regeneration without an attributed upstream cause, is the guard.
- **Live DEM text parity needs a 128-bit `long double` host**, as today.
