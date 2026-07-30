# Goal: Close A6 Before Decoder Extraction

## Objective

Finish milestone A6 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) from one clean reviewed revision before admitting the parked `stab-decoder` A7 work.

## Current State

- A0 through A5 are complete.
- The committed product graph physically contains `stab-bits`, `stab-model`, `stab-records`, `stab-algebra`, `stab-analysis`, `stab-engine`, `stab-kernels-simd`, `stab-core`, and `stab-cli`.
- Commit `95df87ee` removes the accidental model-to-algebra edge and duplicate engine namespace aliases. The current correctness inventory digest is `e32b5b3e4939ed120c42193452fb85d6f73d72225412c2c8b2876739f38b6601`; the performance inventory digest is `cff7389b76971b615485680573ea4a4390becd15ee665f6911915107dd1553b9`.
- Evidence from `2089fab4` is historical after the source and inventory changes above. It proves the prior matrix was executable but cannot close the current revision.
- `stab-decoder` source and benchmark work is preserved in named stashes and must remain parked until A6 closes.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), milestone A6
- [Component graph](../architecture/README.md)
- [A6 extraction map](../architecture/a6-component-extraction-map.md)
- [Progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owner when Cargo metadata, architecture checks, generated inventories, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Commit the amended A6 evidence contract before collecting new evidence.
2. From that clean commit, regenerate full-tier M5 and M6 correctness evidence under the current inventory identities.
3. Regenerate scalar-versus-portable SIMD diagnostics. Keep scalar selected unless a complete confidence interval proves a material benefit; do not claim A9 controlled-host parity.
4. Produce one fresh pinned-Stim baseline and one warmed three-run comparison for all 166 frozen rows. Require all 165 executable rows, the one metadata anchor, exact semantic witnesses, unchanged policies, both source-owned profiler-note roots, and unique artifact paths.
5. Label the 19 source-owned phases without semantically identical clean predecessors as initial seeds. Require passing witnesses and measurements, but make no retrospective regression claim.
6. Run isolated warmup plus one-outer-run diagnostics for every greater-than-15-percent report-only crossing lacking a valid focused artifact, including the eight rows named in the A6 benchmark contract. Use hardware profiling only when host policy permits it.
7. Record exact M5 and M6 scalar Stim pairs, keep M6 short-right-operand and non-identity SIMD evidence distinct, and update the append-only progress report.
8. Run milestone-audit and full-code-review. Fix implementation, evidence, and documentation findings; log only genuine under-specification.
9. Run the full verification matrix and obtain green GitHub CI for the exact closure commit. Do not close A6 or restore A7 work before both local and CI gates pass.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and scalar engine code.
- Only `stab-kernels-simd` contains direct portable-SIMD code, and it has no Stab dependency.
- `stab-core` remains the compatibility facade; canonical implementation and qualification ownership stays with component crates.
- No comparator class, `1.25x` threshold, waiver, semantic witness, or feature-selection policy is relaxed to obtain closure.
- Host-unverified timing is diagnostic only. A9 owns promotable controlled-host parity.
- Historical and failed artifact paths remain immutable and are never reused.
- The staged pre-commit hook counts only when run against the actual staged files of each focused commit.

## Done

A6 is complete only when source-current correctness, SIMD, complete-matrix, focused-crossing, selected scalar, architecture, inventory, audit, review, local verification, and exact-revision CI evidence all pass with no open finding. Then mark A6 complete, commit the synchronized record, and begin A7 by restoring and reviewing the named decoder stashes.
