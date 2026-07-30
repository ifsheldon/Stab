# Goal: Close Modular Boundaries And A6 Evidence

## Objective

Finish the remaining A0, A2, and A3 audit repairs, then close milestone A6 of [the agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) from one clean reviewed revision before implementing A7.

## Current State

- The physical workspace split is complete: `stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, `stab-engine`, `stab-kernels-simd`, the `stab-core` facade, and `stab-cli`.
- A1, A4, and A5 are complete. A0, A2, and A3 have narrow audit repairs in progress around documentation governance, complete compiler discovery, and direct Stable record consumption.
- The retrospective A6 attestation procedure is superseded. Historical complete matrices, focused reports, and publication designs remain historical diagnostics, not current acceptance evidence.
- A6 now requires affected-path evidence: scalar-versus-SIMD XOR and Clifford reports, focused current M5 and M6 qualification diagnostics under the unchanged `1.25x` numeric criteria, direct owner-package qualification, local verification, and exact-revision CI on the measured source commit.
- The two A7 stashes are historical prototypes. They must not be restored wholesale because review found incorrect implementation placement, dynamic dispatch, weak resource admission and oracle gaps, and an incomplete benchmark scaffold.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md)
- [A6 extraction map](../architecture/a6-component-extraction-map.md)
- [Architecture graph and ADRs](../architecture/README.md)
- [Append-only progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owning source when Cargo metadata, architecture checks, generated inventories, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Finish complete engine compiler descriptors, Stable record-consumer evidence, local Markdown-link validation, dependency-graph ADR coverage, and migrated test selectors.
2. Regenerate correctness, performance, and status artifacts; commit product, test, operational, and generated changes in focused commits.
3. Run milestone-audit and full-code-review. Fix every confirmed product, test, benchmark-contract, and documentation finding before timing.
4. From the resulting clean source commit, run Stable and Nightly architecture checks, direct owner-package suites, workspace verification, oracle checks, and benchmark smoke.
5. Produce one source-current full scalar-versus-SIMD report for medium and large XOR and non-identity Clifford workloads using identical inputs and exact output witnesses.
6. Produce full-tier `small`-scale diagnostic qualification reports for `PERFQ-M5-SIMD-BITS` / `xor-complete-vector` and `PERFQ-M6-CLIFFORD-STRING` / `right-multiply-identity`. Require semantic preflight, acceptable noise, median ratio at most `1.25`, and confidence upper bound at most `1.25`; record unverified host status without promoting it.
7. Rerun exactly the finite A2, A4, and A5 diagnostic table in the A6 extraction map. Preserve each existing comparator classification; do not add or omit rows during execution.
8. Record host validity and unique artifact paths, restore swap exactly if timing changed it, and require green CI on the exact measured source revision. A following closure commit may update only the append-only progress report, generated status, specification-gap log, and linked plan status prose; any executable or contract change invalidates the timing.
9. Mark A6 complete only after a final audit. Start A7 from the reviewed contract and conformance requirements, manually reusing sound ideas from the parked prototype rather than applying either stash.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and scalar engine code.
- Only `stab-kernels-simd` contains direct portable-SIMD code, and it has no Stab dependency.
- Runtime capabilities advertise every implemented compiler and explicitly report unavailable request fingerprints or backends.
- `stab-core` remains the compatibility facade; canonical implementation and qualification ownership remain in component crates.
- No comparator, `1.25x` threshold, semantic witness, feature choice, or timing boundary is relaxed to obtain closure.
- Optional profiles are diagnostic only and cannot relabel a failed timing result.
- Historical, failed, and rejected artifacts remain immutable and are never promoted as source-current evidence.

## Done

A6 is complete only when all narrow A0/A2/A3 repairs pass, component boundaries and direct ownership remain green, source-current SIMD and selected M5/M6 diagnostic evidence satisfy their numeric criteria, generated artifacts are synchronized, final audits find no blocker, local verification succeeds, exact-revision CI for the measured source commit is green, swap is restored, and the worktree is clean.
