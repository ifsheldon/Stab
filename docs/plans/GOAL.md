# Goal: Close A8 Circuit Pass And Backend Extension Seams

Status: Active. Local source, correctness, diagnostic, and audit work is complete; exact-revision GitHub CI is the remaining blocker.

## Objective

Close Milestone A8 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) without expanding the earned extension boundary. The common typed circuit-pass executor, built-in without-noise adapter, external Stable noise pass, and sampling-backend selection surface must remain source-current and evidence-backed.

## Measured Source

- Product and qualification source revision: `c797ebc908ce1b81675e479031c39f71740058ae`.
- Correctness inventory identity: `afec1b7090cc1254d6414ec4e10333e3d43976bbb5cc680822797ef231f4c676`.
- Performance inventory identity: `5d35927f8518a6df5de141b674af8d38858b16338437f1e033897b0419090f20`.
- The measured worktree was clean, the pinned Stim revision remained `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, and no failed artifact path was reused.

## Completed Evidence

- Exact A8 correctness selection: 17 source-owned parents, including the repaired analysis resource-identifier owner.
- PR: 17 of 17 passed and replayed at `target/qualification/correctness/a8-c797ebc9-pr-r2`.
- Full: 17 of 17 passed and replayed at `target/qualification/correctness/a8-c797ebc9-full`.
- Soak: 17 of 17 passed and replayed at `target/qualification/correctness/a8-c797ebc9-soak`.
- Controlled external-pass diagnostic: `target/benchmarks/qualification/a8-c797ebc9-external-noise-pass-controlled-pr`, verified host, report SHA-256 `76c9184f3bc5bdc8e3d04bfd230486ff0e70480b6eb3104432b34ac503a6ab15`.
- External-pass medians: `94.108`, `92.936`, and `98.992` ns per represented input instruction at small, medium, and large scales.
- Built-in continuity: current `0.000229623` seconds versus predecessor `0.000232035` seconds, a `0.989605x` ratio. This is report-only continuity evidence, not a Stim parity or self-regression gate.
- Final milestone-audit and full-code-review found no unresolved implementation defect or specification loophole. Existing files near 1200 lines remain a watch list, but A8 introduced no source file over policy.
- `/swap.img` is active with its original size `137438949376` bytes and priority `-2`; no benchmark or qualification process and no A8 temporary worktree remains.

## Remaining Blocker

GitHub still reports `origin/main` at A7 revision `da276e4933aa8ebced4279b77a790b0cb11998a5`. The A8 commits are therefore not present on the remote and no workflow has tested the A8 source. Do not mark A8 complete or relabel the local diagnostics as release evidence until a pushed revision containing `c797ebc9` passes both CI jobs.

## Next Actions

1. Push local `main` to `origin/main` and verify the remote contains `c797ebc9` unchanged as the measured product ancestor.
2. Watch the resulting GitHub Actions run and repair any source-current failure instead of weakening a contract.
3. After both `Rust` and `Qualification Contracts` pass, append the run ID and exact tested revision to the progress report.
4. In one status-only closure commit, mark A8 complete in the architecture plan and feature checklist, then make A9 the active goal.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially Milestone A8
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Component contracts](../architecture/component-contracts.md)
- [Decoder extension ADR](../architecture/adr-0006-decoder-extension-boundaries.md)
- [Correctness qualification contract](comprehensive-correctness-qualification-plan.md)
- [Performance qualification contract](comprehensive-stim-performance-qualification-plan.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Nonnegotiable Closure Rules

- The pass seam stays closed over public typed circuit models; no dynamic plugin ABI, runtime gate registration, public execution IR, external decoder transport, GPU placeholder, or fabricated comparator is added.
- The external-pass diagnostic remains Stab-only. It proves bounded source-current throughput and semantic witnesses, not Stim parity, current self-regression, or release qualification.
- Historical, unverified, failed, and predecessor artifacts keep their original identities and claims.
- Any product, test, benchmark-contract, inventory, fixture, workflow, or substantive normative change after `c797ebc9` invalidates this checkpoint and requires fresh affected evidence.

## Done

A8 is complete only after exact-revision CI is green and the status-only closure records that run without changing the measured product or evidence contracts.
