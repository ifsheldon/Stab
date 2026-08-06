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

## Finding Ledger

Each Pass 1 finding gains a row here when its fix lands: finding, owner, witness, implementing commit, evidence status.

| Finding | Owner | Witness | Commit | Evidence |
| --- | --- | --- | --- | --- |
| Detector frame collapses only the first Pauli term of product measurements | `crates/stab-engine/src/detection/frame.rs` | `HERALDED_ERASE(0) 2` / `R 0 1` / `MXX 0 1` / `MZZ 0 1` / `DETECTOR rec[-1]` never fires; fails pre-fix | `957af5a9` | X/Y/MPP invariant regressions plus 6-sigma joint statistical test, all failing against pre-fix logic via stash run; engine suite green |
| Reference sample applies `p == 1` measurement flips that pinned Stim drops | `crates/stab-engine/src/sampling/measurement_flip.rs` | `MR(1) 0` / `DETECTOR rec[-1]` fires every shot in pinned Stim; Stab inverted pre-fix | `648279a5` | `reference_flip_semantics` suite (reference_sample, detect, m2d, skip-reference; general and direct-Z paths) fails 3/3 pre-fix; detect/m2d outputs verified against the pinned Stim binary; core, cli, engine, M8, and M9 oracle lanes green |
| Public count/reference helpers panic on parseable circuits; count semantics diverge from Stim; MPAD inflates public qubit counts | `crates/stab-engine/src/sampling/{mod,execute,direct_z_measurement}.rs`, `crates/stab-core/src/sampling.rs`, `crates/stab-model/src/circuit/counts.rs` | `M 16000000` returns a typed error through every public entry point; `M(0.5) 0` counts determined like Stim; `MPAD`/heralded reject like Stim's unhandled-type throw; `count_qubits("H 0\nMPAD 1")` is 1 per `circuit_instruction.cc:64-69` | (this change set) | Panicking wrappers deleted; flip arguments ignored in the count path with physical-outcome reset conditioning; one qubit-count owner; new engine pins for flip-ignoring and typed rejection; renamed core pin corrected against vendor source; MIGRATING-0.2.md records the API changes; model, engine, core, cli, analysis, records, decoder, bits, and algebra suites green |

| Bare `REPEAT` accepted as an instruction; Stim-legal spaced combiners rejected; `convert --in_format` silently defaulted | `crates/stab-model/src/circuit/parser.rs`, `crates/stab-model/src/target.rs`, `crates/stab-model/src/gate/mod.rs`, `crates/stab-cli/src/convert.rs` | `REPEAT` parses pre-fix where pinned Stim errors "Missing '{'"; `MPP Z0 *Z1` rejects pre-fix where pinned Stim accepts; `convert` without `--in_format` emitted output pre-fix where pinned Stim exits 1 | `49f87330` | `stim_grammar_remediation` suite plus the required-flag regression (exit status, stderr class, untouched output path); combiner accept/reject set matched to the pinned binary probe matrix; parser fuzz smoke green; model, cli, core, engine, and analysis suites green |
| Draft verification queries the published-only by-tag release endpoint, so `create-draft` post-upload verification and `verify-remote-draft` fail with HTTP 404 against a real draft | `ops/release/src/github.rs` | Against a GitHub-faithful mock that returns 404 for by-tag draft lookups, the pre-fix lookup fails with `GitHub release query returned HTTP 404 Not Found` while the release-list lookup finds the draft | `963024ea` | Draft verification now scans the paginated release list under a fixed page bound and requires exactly one matching draft; six new wire and routing tests cover the 404-faithful witness, state routing, pagination, exactly-one enforcement, bound fail-closed behavior, and the retained published-state by-tag path; `stab-release` suite green ten of ten runs after the pre-existing ETXTBSY test race was repaired in `ca4b030b`; the scratch-repository rehearsal required by WS4 success criterion 2 stays pending until the release freeze lifts |

### Pending within Batch A

- Committed oracle fixture manifest rows for the `M(1)`/`MR(1)`, bare-`REPEAT`, spaced-combiner, and `convert --in_format` witnesses (one manifest-editing pass).

## Deferred Backlog

The plan's WS6 items 2 through 6 and 8, WS7, and WS8 are deferred behind the promotion triggers named in the plan's Execution Overlay; none have been promoted at this entry.
