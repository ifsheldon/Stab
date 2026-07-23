# Goal: Repair Compatibility And Qualification Evidence

## Status

Active execution contract as of 2026-07-23.

The complete task specification is [post-review-compatibility-evidence-repair.md](post-review-compatibility-evidence-repair.md). Read [lessons-learned.md](lessons-learned.md), `docs/AGENTS.md`, and `benchmarks/AGENTS.md` before changing their owned surfaces.

## Implementation Checkpoint

The R0 through R5 source, test, schema, inventory, and documentation changes are committed in a focused source series. The checked correctness inventory has digest `592934174f3cf248553d3df67078ec00563e48acfd4c5ddf15cef44fd9b49fd0`; the checked performance inventory has digest `33b796a2eda59429fcccc43a3db8dc715608e5dffabd9cfe1b756c4d40529358`. Both inventories contain 2,065 public API items, and the performance gates remain unchanged at `1.25x`.

Commit authorization was received and the clean-source prerequisite is satisfied after this checkpoint commit. R6 has not started; its formal correctness, timing, memory, DEM, rollup, and completion artifacts must use the final clean unchanged `HEAD`, bind `local_modifications=false`, and use new artifact paths.

R7 source review and verification are tracked in [post-review-compatibility-evidence-repair-progress-report.md](post-review-compatibility-evidence-repair-progress-report.md). Execute R6 next from the clean checkpoint revision, then repeat the audits and final verification against the resulting evidence.

## Mission

Close every verified external-review finding and the Rust qualification timing defect without weakening Stim v1.16.0 compatibility, the `1.25x` performance gates, or evidence provenance.

Path-alias data loss and result-format incompatibility are release blockers. The DEM chain produced at `80fb5405fb077c694a8a8a18e64a3a5831e20a5e` is retained as immutable historical `raw-work-v1` evidence and is review-rejected as current evidence.

## Required Order

1. Freeze affected public claims and historical evidence status.
2. Add failing path-alias regression tests, then centralize CLI file ownership and preflight.
3. Add failing grammar and DETS semantic tests, then implement the shared byte lexer and typed DETS APIs.
4. Add the pinned-Stim differential corpus and independently selectable qualification owners, then regenerate inventories.
5. Replace the legacy benchmark runner with the bounded shared supervisor.
6. move dispatch and all post-work processing outside timing, add `raw-work-v2`, and bump every affected schema.
7. Run targeted and workspace verification on the dirty tree.
8. Stop before promotable evidence until the user explicitly authorizes focused commits.
9. After authorization, commit source and contract changes, then produce correctness and performance evidence from one clean unchanged revision and unique artifact paths.
10. Synchronize documentation, run milestone audit and full code review, fix all confirmed findings, and complete final verification.

Do not reorder a dependent milestone merely because a later check is easier to run.

## TDD Contract

For each behavior:

1. Add an independently selectable regression test.
2. Confirm it fails for the intended defect when practical.
3. Implement the narrowest complete fix through the shared boundary.
4. Run targeted tests for every consumer of that boundary.
5. Add or update pinned-Stim differential evidence.
6. Update public API, CLI, oracle, qualification, benchmark, and user documentation in the same change set.

Round trips are supporting tests only. They cannot own compatibility when Stab's reader and writer could agree with each other while disagreeing with Stim.

## Compatibility Contract

- Target pinned Stim v1.16.0 only.
- Existing width-based DETS readers remain callable but are measurement-only and reject `D` and `L`.
- Layout-aware DETS readers use independent M/D/L namespaces.
- Dense and packed DETS duplicates set bits; typed sparse visitors preserve duplicates; only `SparseShot` observable masks apply parity.
- HITS dense duplicate parity remains unchanged.
- `01` and HITS require LF or CRLF after every record.
- All public text readers share one byte-exact grammar implementation.
- Explicit CLI input/output and output/output aliases are rejected before truncation, including relative, symlink, and hardlink aliases.
- Input/input aliases remain valid.
- Shell redirection is outside the explicit path-flag contract.
- Intentionally deferred Stim APIs and ecosystem surfaces remain deferred.

## Evidence Contract

- Keep all gates at `1.25x`.
- Do not add waivers for regressions introduced or revealed by this work.
- Never reuse an artifact path.
- Preserve failed and historical artifacts with their exact source and schema identity.
- Never relabel `raw-work-v1` as `raw-work-v2`.
- Promotable evidence requires `local_modifications=false` before and after every producer.
- Do not produce formal evidence until the user authorizes the focused source commits required to create that clean revision.
- Disable swap only for formal timing and restore the exact prior configuration on every exit path.

## Milestone Acceptance Loop

For each milestone:

1. Check every task, linked test, benchmark, schema, resource boundary, and done criterion in the active plan.
2. Run the milestone's targeted verification.
3. Run `milestone-audit`.
4. Fix implementation and evidence findings. Log only genuine newly revealed under-specification in `milestone-spec-gaps.md`.
5. Run `full-code-review` over the touched compatibility, CLI, process, qualification, benchmark, test, and documentation surfaces.
6. Fix every confirmed finding and rerun affected checks.
7. Record exact commands, outcomes, revisions, inventory digests, and unique artifact paths in the progress report.

## Completion Conditions

This goal is complete only when:

- every explicit path-role conflict fails before truncation;
- all result-format readers and CLI consumers match the checked pinned-Stim corpus;
- every new public API has exact independently selectable ownership;
- the benchmark tree uses one bounded process supervisor;
- both workers and all derived evidence bind `raw-work-v2`;
- affected correctness qualification passes;
- primary timing and memory checks pass without new waivers;
- the twelve DEM reports, twelve regressions, four rollups, two completions, and two accepted-maximum memory probes pass and replay from one clean revision;
- README, checklist, plans, inventories, benchmark docs, and progress reports agree;
- milestone audit and full code review have no unresolved confirmed finding;
- formatting, Clippy, workspace tests, oracle checks, qualification checks, benchmark smoke, and pre-commit pass;
- swap is restored, no qualification process remains, and the worktree is clean.
