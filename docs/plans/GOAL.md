# Goal: Close Stable Records Extraction

## Objective

Finish milestone A3 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) from one clean committed revision, then activate A4. Do not begin the sampling plan/session split while any A3 implementation, evidence, review, or documentation blocker remains open.

## Current State

- A0, A1, and A2 are complete.
- `stab-bits` is a committed Stable Rust 1.97.1 crate.
- `stab-records` is physically extracted and depends only on `stab-bits`.
- `stab-core` depends on both leaf crates and preserves compatibility re-exports.
- All 62 checked result-format corpus cases pass through the extracted APIs and pinned Stim.
- Typed shot-major, bit-plane, measurement, detection, observable, sampled-error, and sink boundaries exist.
- Dense and packed HITS/DETS readers use event-driven parsing with width-bounded scratch even for duplicate-heavy records.
- Raw sparse and typed-token visitors preserve one record's duplicates and order by contract.
- Legacy `MeasureRecordWriter` and `MeasureRecordBatchWriter` remain documented compatibility adapters; new component code uses typed sinks and typed DETS namespaces.
- The extraction is committed at `46abdac2`; its benchmark contracts are committed at `b8dff63c`.
- Existing dirty benchmark reports remain diagnostic only; clean source-current evidence is the remaining A3 closure blocker.

## Sources Of Truth

- Active plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), milestone A3
- Progress record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Component contracts: [../architecture/component-contracts.md](../architecture/component-contracts.md)
- Migration inventory: [../architecture/0.2-api-migration-inventory.md](../architecture/0.2-api-migration-inventory.md)
- Specification gaps: [milestone-spec-gaps.md](milestone-spec-gaps.md)
- Generated status: [../qualification-status.md](../qualification-status.md)

Stop and repair the owning source when code, tests, generated inventories, benchmark contracts, or these documents disagree.

## Closure Sequence

1. Keep the extracted crate graph and Stable toolchain boundary clean.
2. Finish direct codec, typed sink, cancellation, duplicate-heavy allocation, and compatibility-facade tests.
3. Regenerate and validate correctness ownership, performance dispositions, and generated status.
4. Preserve the completed milestone-audit and full-code-review result: no P0/P1 finding remains, and all touched code files are below 1,200 lines.
5. Commit this synchronized documentation checkpoint.
6. From that clean revision, rerun the three A3 component rows plus representative `01`, `b8`, HITS, DETS, PTB64, and convert rows using unique artifact paths.
7. Compare source-current observations with the recorded pre-extraction baseline where workloads are identical. Label unmatched component rows report-only and make no invented Stim or pre/post ratio.
8. Record exact revision, artifact paths, work units, allocation observations, and any explained noise in the append-only progress report.
9. Rerun all required checks on the final documentation commit and require a clean worktree.

## Done Criteria

- Cargo metadata proves `stab-records -> stab-bits` and no forbidden product edge.
- Both leaf crates build and test on Rust 1.97.1.
- Exact bytes and all 62 pinned compatibility cases pass.
- Typed layouts and sinks, first-error cancellation, and bounded working scratch have direct tests.
- Correctness and performance inventories regenerate byte-for-byte and have no planned owner for implemented records behavior.
- Clean source-current benchmark evidence has truthful work units and no unexplained material regression.
- Milestone-audit, full-code-review, workspace verification, and pre-commit have no open A3 finding.

## Required Checks

```text
cargo +1.97.1 check -p stab-bits -p stab-records
cargo +1.97.1 test -p stab-bits -p stab-records
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
just architecture::check
just oracle::result-formats --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```
