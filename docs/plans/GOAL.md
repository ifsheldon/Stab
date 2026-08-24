# Goal: Reclaim Pre-0.2 Code Entropy

Status: Active pending exact-head GitHub CI. Local E0 through E7 implementation, entropy verification, milestone audit, full code review, and final checks are complete on `main`; the release-oriented A9 sequence remains paused, and no timing from this simplification revision is promotable.

## Objective

Complete E0 through E7 without weakening Stim compatibility, data-loss prevention, subprocess safety, descriptor-safe evidence publication, benchmark equivalence, the `1.25x` parity gate, or the `1.15x` Stab regression policy. Work directly on `main` in focused commits, then resume A9 from one clean exact-head revision.

## Current State

- The project-local `reclaim-code-entropy` skill and main-checkout policy are installed.
- Historical A9 rehearsal records remain valid history; the deleted scratch repository's workflow, binary, target, recipes, and active release requirements have been retired.
- Performance inventory schema 5 is compact, and runtime-group schema 11 is the sole detailed owner for executable groups and scales. The ambiguous parity alias and duplicate regeneration-check routes are removed.
- Unused core and bit error variants are removed. Reverse-flow classification, sparse tracking, and recurrence machinery remain live implementation details but are no longer exported through `stab_analysis::advanced`.
- The checked inventories are current; [the generated qualification dashboard](../qualification-status.md) is the sole owner of their counts, digests, and completion checkpoint.
- The superseded A6 focused-attestation implementation and hidden commands are removed. Its two retained JSON contracts are archived byte-for-byte under `benchmarks/archive/a6/`; active SIMD compare and report commands are unchanged.
- E5 now has one oracle compatibility-matrix parser, one CLI result-format enum, one private DEM-search mask/index representation, model-owned target text assertions, and corpus-owned malformed CLI cases.
- Audit fixes restored corpus-owned mixed-layout DETS coverage to measurement-only readers, `m2d`, and replay; documented every public removal; replaced duplicate qualification selectors with distinct semantic tests; retired the stale rehearsal gap; and removed the last speculative benchmark states and rehearsal dispatch seams.
- The complete local E7 sequence passes, including the 313-row matrix, live 62-case result-format differential, all implemented oracle fixtures, both qualification contracts, and the two required adapter probes. The probes are diagnostic only and do not satisfy formal A9 timing.
- No formal timing from an intermediate simplification revision may be promoted.

## Execution Order

1. E0 and E1 are complete: the contract is frozen and the scratch release lane is historical only.
2. E2 is complete: the speculative detailed backlog and mirrored API/checklist/group contracts are gone, while workload and regression identities remain intact.
3. E3 is complete: approved unused public errors and advanced-analysis exports are gone, while consumed private engines and public cross-crate helpers remain.
4. E4 is complete: A6 JSON contracts are archived and the superseded executable attestation lifecycle is gone.
5. E5 is complete: the proved duplicate parsers, CLI format enum, private DEM-search value types, and tests are consolidated without dropping semantic coverage.
6. E6 is complete: CI checks the canonical contracts, volatile status lives in the generated dashboard, and retired rehearsal and A6 procedures are explicitly historical.
7. E7 is locally complete. Push the focused `main` commits, require GitHub CI to pass that exact commit, then resume A9 from a new immutable evidence path.

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
