# PQ2 Completion Receipt Infrastructure Progress Report

> Current-use note, 2026-07-16: the three `not_zero` groups and both sparse-XOR groups have since published and byte-for-byte replayed schema-version-1 completion receipts. This report remains the infrastructure acceptance record; the product evidence lives in their respective PQ2 progress reports.

Date: 2026-07-16

Status: Complete for the machine-readable completion-receipt infrastructure.

Implementation revision: `b208a359f3f7676e2b07d64a5dc8caca208abf6a`.

This report closes the under-specification recorded in `docs/plans/milestone-spec-gaps.md` for machine-readable PQ2 closure evidence. It does not qualify another product workload, create a new Stim-relative ratio, relabel historical dense-XOR evidence, or mechanically certify milestone audit and independent review.

## Implemented Contract

- `just bench::qualification-completion` executes private-worker reproducibility, the exact source-owned adapter probe, idempotent source-report replay, source-owned regression validation, and full and soak rollup replay through typed Rust handlers.
- Completion receipt schema version 1 binds the clean Stab revision, frozen Stim identity, performance and correctness inventories, runtime-group contract, canonical standalone argument vectors, exact input and output artifact identities, typed results, and zero exit status only after successful handler completion.
- The receipt requires one promotable full and one promotable soak report per source-owned scale, passing complete measurement gates, passing full and soak rollups, one exact CPU and host identity, one exact correctness preflight, and one reproducible worker pair.
- `just bench::qualification-completion-report` reruns every machine-checkable operation and requires byte-identical canonical receipt and preflight reconstruction.
- Shared qualification publication retains validated source and replay-target directory descriptors through commit, rejects byte-identical directory-inode substitution, verifies parent and staging identities, makes replacement durable before bounded old-tree cleanup, and treats cleanup failure as best effort only after durable publication.
- The requirement is prospective for executable PQ2 closures claimed after schema version 1 was introduced. The fifth dense-XOR slice remains historical under its preceding acceptance contract.

## Test Evidence

The completion module has direct tests for canonical receipt structure, typed step order, path and tier substitution, preflight binding, probe and worker identity, group eligibility, exact rollup sources and outcomes, mixed CPU rejection, handler order, first-error termination, non-idempotent replay, final source-directory substitution, replay-target substitution, and cleanup failure after durable publication.

The shared artifact layer has direct tests for parent, staging, source, target, and exchanged-directory identity, successful bound-target refresh, stale refresh rejection, unexpected artifacts, bounded reads, source digest binding, atomic replacement, and cleanup failure.

Focused results at the implementation revision:

- Artifact publication tests: 12 passed.
- Completion receipt and workflow tests: 15 passed.
- Benchmark harness tests: 259 total, with 257 passed and 2 intentional subprocess or private-build ignores.

## Milestone Audit

Status: Complete.

No implementation defect or remaining specification loophole blocks this infrastructure milestone.

| Requirement | Status | Evidence |
| --- | --- | --- |
| Typed machine-checkable closure sequence | Satisfied | `ops/bench/src/qualification/runtime/completion/workflow.rs` and sequencing tests |
| Exact receipt, preflight, and replay binding | Satisfied | `completion/model.rs`, `completion/validation.rs`, and canonical reconstruction tests |
| Passing report, regression, and rollup gates | Satisfied | Production handlers in `completion/workflow.rs` and adversarial validation tests |
| Exact worker, CPU, host, toolchain, and correctness identity | Satisfied | Completion environment and rollup identity validation plus mismatch tests |
| Atomic hostile-path publication boundary | Satisfied | `runtime/artifact.rs` and production-path directory-substitution tests |
| Prospective acceptance policy | Satisfied | `GOAL.md`, the performance plan, the dense-XOR report, and the specification-gap resolution |
| Human audit and review kept separate | Satisfied | Receipt Markdown and operational documentation |

The real completion and replay commands were not run against the five historical product groups because the receipt contract is prospective and requires source-current full and soak evidence from one clean committed revision. Later `not_zero` and sparse-XOR slices exercised both commands before claiming closure, without retroactively relabeling the first five groups as receipt-backed.

## Independent Full Code Review

The first GPT-5.6/max review found four issues: cleanup could fail before durable publication, exact CPU identity was not bound, tests did not exercise the orchestration and final-publication seams deeply enough, and the documentation could be read as retroactively applying the new receipt to dense XOR. Those issues were fixed.

The second pass found that validated source and replay-target directory descriptors were not retained through the final commit boundary and that the replacement tests did not reach the production publication path. The artifact layer and completion publication seam were hardened, and byte-identical inode-substitution tests were added.

The final GPT-5.6/max re-review reported no remaining P1 or P2 finding. It confirmed coherent ownership between command dispatch, orchestration, receipt modeling, validation, artifact publication, report replay, regression, rollup replay, probes, and worker reproducibility, with no reviewed source over 1,200 lines.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just bench::qualification-check`
- `just bench::smoke`
- `just maintenance::pre-commit`

All required checks passed. The staged large-file check reported five watch-list files between 900 and 1,200 lines and no file at or above the 1,200-line limit.

## Next Action

Select the next finite source-owned PQ2 workload and use schema-version-1 completion publication and replay as part of its initial acceptance evidence. Preserve the already accepted `not_zero` and sparse-XOR receipts as historical evidence when later shared-worker or inventory changes occur; do not relabel them as current-inventory reports.
