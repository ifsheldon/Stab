# Post-Review Remediation Progress Report

Rolling progress record for [post-review-remediation-plan.md](post-review-remediation-plan.md).
Update this file at each batch completion with the metrics required by the plan's Progress Reporting section.

## Identity

- Review base revision (revision the August 2026 full code review examined): `19266c0e39e8935cf420fe62c775f054e0fdd0ef`.
- Remediation base revision (amended plan committed): `d5c04300` (`docs(plans): amend remediation plan after second-review convergence`).
- Current head: updated per entry below.

## Release-Freeze State

- Frozen since Gate 0 (this entry): A9 evidence production, completion checkpoints, package preflight, crates.io publication, `v0.2.0` tag creation, and GitHub draft or release creation are prohibited per the Remediation Freeze section of [GOAL.md](GOAL.md).
- The freeze lifts only when the plan's Pass 1 closes and a later entry in this report records the restoration change set.

## Batch Log

### Gate 0 (P0.0): release-authorization freeze and source-of-truth synchronization

- State: complete at this entry's commit.
- Changes: `GOAL.md` reopened (status, superseded no-P0/P1 claim, Remediation Freeze section, gated Next Actions); 19 checklist rows flipped to `Reopened` and 2 `Partial` rows annotated with withdrawn claims, each naming its remediation workstream; PQ0 performance ledger and `qualification-status.md` regenerated from the updated checklist; this report created.
- Evidence invalidated: all prior qualification claims for the reopened rows are withdrawn; no new evidence produced (documentation-only change set).
- Findings addressed: none yet (product fixes begin in Batch A).
- Commands and results: recorded in the Gate 0 commit message and reproduced below after regeneration.

### Batch A (Pass 1): WS1, WS3 items 1 through 3, WS4 item 1

- State: complete at this entry's commit; WS1 passed the independent milestone audit required by the plan before a completion declaration.
- Product commits: `957af5a9` (whole-product collapse), `648279a5` (strictly noiseless reference samples), `c5802b13` (Stim-aligned determined counts, panicking-wrapper deletion, MPAD counting), `49f87330` (bare-`REPEAT` rejection, spaced combiners, required `convert --in_format`), `963024ea` (draft verification through the release list) with `ca4b030b` repairing the pre-existing ETXTBSY test race in the same crate.
- Evidence commits: `2261cd5d` and `dfc9b878` added seven committed exact-output oracle rows (`m9-detect-flip-reference`, `m9-detect-flip-reference-dets`, `m9-m2d-flip-reference`, `m8-sample-skip-reference-flip`, `m4-parser-bare-repeat-reject`, `m4-parser-spaced-combiners-accept`, `m7-convert-missing-in-format-reject`), all passing stim-stab parity with `record --check-clean` stable; `80d62ed6` reconciled the CQ0 inventory with the fallible count API after the sweep gap `c5802b13` left open.
- Inventory digests: correctness `afec1b70...` to `b7a8fc12...`; performance `5d35927f...` to `74c204a7...`; prior CQ1/PQ1 evidence over the old digests is historical per the documented digest policy, and the rerun is owned by the freeze-lift change set.
- WS1 milestone audit: verdict complete with spec follow-ups; the auditor independently reverted both headline fixes and reproduced the failures (2047 of 4096 shots firing under first-term collapse; all three `reference_flip_semantics` tests failing under the flip-applying reference). Follow-ups applied in this entry's commit: checklist rows `Compiled measurement sampling` and `stim sample` re-attributed to WS3-only causes, an MPAD construction-path rejection pin added beside the existing TICK pin, criteria 2 through 4 and reference task 2 amended in the plan's Amendment Record revision 2, the `M 16000000` ledger claim corrected, and the panicking `CompiledDetectionConverter::reusable_*` compatibility wrappers recorded as a WS5 deletion item.
- WS4 item 1 residue: the scratch-repository rehearsal required by WS4 success criterion 2 stays pending until the release freeze lifts.
- Verification: engine, model, core, cli, analysis, records, decoder, bits, and algebra suites green; M4, M7, M8, and M9 oracle lanes pass; `oracle::record --check-clean`, `oracle::matrix --check`, `oracle::list`, `qualification::correctness-check`, `bench::qualification-check`, `qualification::status --check`, and the documentation link check all pass at this entry's head.

## Finding Ledger

Each Pass 1 finding gains a row here when its fix lands: finding, owner, witness, implementing commit, evidence status.

| Finding | Owner | Witness | Commit | Evidence |
| --- | --- | --- | --- | --- |
| Detector frame collapses only the first Pauli term of product measurements | `crates/stab-engine/src/detection/frame.rs` | `HERALDED_ERASE(0) 2` / `R 0 1` / `MXX 0 1` / `MZZ 0 1` / `DETECTOR rec[-1]` never fires; fails pre-fix | `957af5a9` | X/Y/MPP invariant regressions plus 6-sigma joint statistical test, all failing against pre-fix logic via stash run; engine suite green |
| Reference sample applies `p == 1` measurement flips that pinned Stim drops | `crates/stab-engine/src/sampling/measurement_flip.rs` | `MR(1) 0` / `DETECTOR rec[-1]` fires every shot in pinned Stim; Stab inverted pre-fix | `648279a5` | `reference_flip_semantics` suite (reference_sample, detect, m2d, skip-reference; general and direct-Z paths) fails 3/3 pre-fix; detect/m2d outputs verified against the pinned Stim binary; core, cli, engine, M8, and M9 oracle lanes green |
| Public count/reference helpers panic on parseable circuits; count semantics diverge from Stim; MPAD inflates public qubit counts | `crates/stab-engine/src/sampling/{mod,execute,direct_z_measurement}.rs`, `crates/stab-core/src/sampling.rs`, `crates/stab-model/src/circuit/counts.rs` | `M 16000000` returns `Ok` through the sparse direct-Z path by design while general-path variants (`H 16000000` / `M 16000000`, huge measurement counts) return a typed storage error through every public entry point; `M(0.5) 0` counts determined like Stim; `MPAD`/heralded reject like Stim's unhandled-type throw; `count_qubits("H 0\nMPAD 1")` is 1 per `circuit_instruction.cc:64-69` | `c5802b13` | Panicking wrappers deleted; flip arguments ignored in the count path with physical-outcome reset conditioning; one qubit-count owner; new engine pins for flip-ignoring and typed rejection; renamed core pin corrected against vendor source; MIGRATING-0.2.md records the API changes; model, engine, core, cli, analysis, records, decoder, bits, and algebra suites green; the CQ0 public-API sweep gap this commit left open was closed by `80d62ed6` |

| Bare `REPEAT` accepted as an instruction; Stim-legal spaced combiners rejected; `convert --in_format` silently defaulted | `crates/stab-model/src/circuit/parser.rs`, `crates/stab-model/src/target.rs`, `crates/stab-model/src/gate/mod.rs`, `crates/stab-cli/src/convert.rs` | `REPEAT` parses pre-fix where pinned Stim errors "Missing '{'"; `MPP Z0 *Z1` rejects pre-fix where pinned Stim accepts; `convert` without `--in_format` emitted output pre-fix where pinned Stim exits 1 | `49f87330` | `stim_grammar_remediation` suite plus the required-flag regression (exit status, stderr class, untouched output path); combiner accept/reject set matched to the pinned binary probe matrix; parser fuzz smoke green; model, cli, core, engine, and analysis suites green |
| Draft verification queries the published-only by-tag release endpoint, so `create-draft` post-upload verification and `verify-remote-draft` fail with HTTP 404 against a real draft | `ops/release/src/github.rs` | Against a GitHub-faithful mock that returns 404 for by-tag draft lookups, the pre-fix lookup fails with `GitHub release query returned HTTP 404 Not Found` while the release-list lookup finds the draft | `963024ea` | Draft verification now scans the paginated release list under a fixed page bound and requires exactly one matching draft; six new wire and routing tests cover the 404-faithful witness, state routing, pagination, exactly-one enforcement, bound fail-closed behavior, and the retained published-state by-tag path; `stab-release` suite green ten of ten runs after the pre-existing ETXTBSY test race was repaired in `ca4b030b`; the scratch-repository rehearsal required by WS4 success criterion 2 stays pending until the release freeze lifts |
| `gen` headers diverge byte-wise for scientific and seven-digit probabilities; Stim-legal `REPEAT(args)` headers reject; `E`/`ELSE_CORRELATED_ERROR` reject decorations pinned Stim ignores | `crates/stab-cli/src/lib.rs`, `crates/stab-model/src/{circuit/parser.rs,circuit.rs,gate/mod.rs,ids.rs}`, `crates/stab-engine/src/{sampling/mod.rs,detection/frame.rs}`, `crates/stab-analysis/src/circuit_to_dem{.rs,/reverse_fold.rs}` | Pinned Stim prints `1e-05`/`0.123457` where Stab printed raw Rust floats; `REPEAT(0.5) 3 {` and every decorated `E` spelling rejected pre-fix where pinned Stim accepts; ten-spelling three-command parity matrix byte-matches post-fix | `f98e4c63` | `Probability::stim_text` owns the six-significant-digit form; REPEAT headers lex and discard arguments through the shared parser; every correlated-error consumer skips combiners like the vendor frame simulator; target writers mirror Stim's `write_targets` including dangling and doubled combiners; the tokenizer waives spacing before combiners; a stale inverted-Pauli cq2 pin was corrected against the probed binary; four oracle rows (`m7-gen-probability-header-*`, `m4-parser-repeat-args-accept`, `m4-parser-correlated-error-decorations`) plus fold-path DEM-equality and parser-fuzz smoke |
| Broken pipes report errors where pinned Stim dies silently; legacy mode flags only work first; detect suppresses the deprecation warning; sample_dem routing flags visible; `FlexPauliString` accepts doubled signs | `crates/stab-cli/src/{lib.rs,diagnostics.rs,detection.rs,sample_dem.rs}`, `crates/stab-algebra/src/pauli.rs` | `stab sample \| head -c 1` exited 1 with a diagnostic pre-fix where pinned Stim exits 141 silently; `--shots 2 --sample` rejected pre-fix where pinned Stim samples; `+-X` parsed pre-fix (stash-run proven) where pinned Stim rejects | `1150bf5c` | Broken-pipe chain walk with silent-141 and kept-diagnostic regressions; single-mode relocation with adjacent-count handling and the `m8-sample-legacy-mode-flag-position` oracle row; warning-before-error pinned byte-exactly in human mode and order-pinned in JSON; hidden-but-functional sample_dem flags per D4; five doubled-sign rejections mirroring Stim; six checklist rows restored; MIGRATING-0.2.md updated |

### Pending within Batch A

- None; Batch A closed with the Batch A entry above, and the WS4 scratch-repository rehearsal is tracked there as a freeze-lift prerequisite rather than a Batch A item.

## Deferred Backlog

The plan's WS6 items 2 through 6 and 8, WS7, and WS8 are deferred behind the promotion triggers named in the plan's Execution Overlay; none have been promoted at this entry.
