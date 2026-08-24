# Goal: Reclaim Pre-0.2 Code Entropy

Status: Active. The release-oriented A9 sequence is temporarily paused while [the pre-0.2 entropy plan](pre-0.2-code-entropy-reclamation-plan.md) removes proved-unused contracts and duplicate ownership. Formal repaired-contract timing has not started for this revision.

## Objective

Complete E0 through E7 without weakening Stim compatibility, data-loss prevention, subprocess safety, descriptor-safe evidence publication, benchmark equivalence, the `1.25x` parity gate, or the `1.15x` Stab regression policy. Work directly on `main` in focused commits, then resume A9 from one clean exact-head revision.

## Current State

- The project-local `reclaim-code-entropy` skill and main-checkout policy are installed.
- Historical A9 rehearsal records remain valid history; the deleted scratch repository's workflow, binary, target, recipes, and active release requirements have been retired.
- Performance inventory schema 5 is compact, and runtime-group schema 11 is the sole detailed owner for all 28 executable groups and 94 scales. The ambiguous parity alias and duplicate regeneration-check routes are removed.
- Unused core and bit error variants are removed. Reverse-flow classification, sparse tracking, and recurrence machinery remain live implementation details but are no longer exported through `stab_analysis::advanced`.
- The regenerated correctness inventory contains 7,591 public API items and 2,203 evidence cases under digest `14614b21280044da7571243b7bb0d5a5e941edd5070999c2f08d98f6e3e923bc`; the compact performance inventory is rebound under digest `88d1e9fcfab579bd2420fbb454fe8d6b57be88d28252f70f2520cc8d10c00bb2`.
- The superseded A6 focused-attestation implementation and hidden commands are removed. Its two retained JSON contracts are archived byte-for-byte under `benchmarks/archive/a6/`; active SIMD compare and report commands are unchanged.
- E5 now has one oracle compatibility-matrix parser, one CLI result-format enum, one private DEM-search mask/index representation, model-owned target text assertions, and corpus-owned malformed CLI cases.
- No formal timing from an intermediate simplification revision may be promoted.

## Execution Order

1. E0 and E1 are complete: the contract is frozen and the scratch release lane is historical only.
2. E2 is complete: the speculative detailed backlog and mirrored API/checklist/group contracts are gone, while workload and regression identities remain intact.
3. E3 is complete: approved unused public errors and advanced-analysis exports are gone, while consumed private engines and public cross-crate helpers remain.
4. E4 is complete: A6 JSON contracts are archived and the superseded executable attestation lifecycle is gone.
5. E5 is complete: the proved duplicate parsers, CLI format enum, private DEM-search value types, and tests are consolidated without dropping semantic coverage.
6. E6 is next: synchronize CI, generated status, release docs, benchmark docs, and architecture docs.
7. E7: run entropy verification, milestone-audit, full-code-review, complete local verification, and exact-head CI.

## Non-Negotiable Gates

- Keep the strict result-format corpus, typed DETS grammar, path-alias safety, bounded process supervisor, independent Stim comparator, active SIMD comparison, production release workflow, and legacy M12 diagnostics.
- Preserve historical reports, tags, digests, and failed artifacts. Mark superseded procedures; do not rewrite their outcomes.
- Do not add compatibility shims for removed pre-`0.2.0` APIs.
- Do not create a branch or linked worktree, relax thresholds, add waivers, or run formal controlled-host timing during source refactoring.
- Run targeted checks for each focused commit and the complete E7 verification from the final clean revision.

## Sources Of Truth

- [Entropy reclamation plan](pre-0.2-code-entropy-reclamation-plan.md)
- [Architecture plan](agent-native-modular-qec-architecture-plan.md)
- [Correctness contract](comprehensive-correctness-qualification-plan.md)
- [Performance contract](comprehensive-stim-performance-qualification-plan.md)
- [Generated qualification status](../qualification-status.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Done

The entropy program is complete only when E0 through E7 pass, active source and documentation contain no retired route or duplicate owner, all audits and final checks pass on exact `main`, GitHub CI passes that exact head, no linked worktree or qualification process remains, and the worktree is clean. A9 then resumes from that final revision using entirely new evidence paths.
