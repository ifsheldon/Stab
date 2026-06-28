# M1 Completion Report

## Milestone

M1: Feature Parity Inventory And Acceptance Contracts.

Objective: create the compatibility contract that later agents implement against instead of guessing from upstream source files.

## Status

Complete for the current Stim v1.16.0 compatibility contract.

M1 is intentionally implementation-light.
Its durable artifact is the source-owned compatibility matrix, which maps upstream Stim docs, C++ tests, Python semantic-mining sources, benchmark sources, Stab owner crates, planned milestones, parity modes, comparator types, priorities, statuses, deferrals, and acceptance checks.

## Tests And Artifacts Ported Or Created

- Added `oracle/compatibility-matrix.csv` as the machine-readable compatibility matrix.
- Added matrix validation in `ops/oracle/src/matrix.rs`, including coverage checks for P0, P1, and Bench upstream files from `docs/plans/stim-test-porting-plan.md`.
- Added `just oracle::matrix --check` to validate matrix coverage and acceptance metadata.
- Added `just oracle::matrix --milestone Mx` to print milestone-specific task lists with parity mode and comparator type.
- Recorded explicit future buckets and deferral reasons for ecosystem surfaces outside the Rust/CLI core scope.

## Implementation Areas

- `docs/plans/stim-test-porting-plan.md` defines the upstream test hierarchy used as the matrix source of truth.
- `oracle/compatibility-matrix.csv` records 313 compatibility rows.
- `ops/oracle/src/matrix.rs` parses, validates, summarizes, and filters compatibility rows.
- `justfiles/oracle.just` exposes the matrix check and milestone filtering workflow.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Build a machine-readable compatibility matrix from Stim v1.16.0 docs, tests, file formats, CLI references, and the porting plan | Satisfied | `oracle/compatibility-matrix.csv`; `docs/plans/stim-test-porting-plan.md`; `just oracle::matrix --check` |
| Give every row an upstream source path, owner crate, planned milestone, parity mode, comparator type, priority, and status | Satisfied | Matrix CSV columns and `ops/oracle/src/matrix.rs` validation |
| Cover planned core surfaces | Satisfied | Matrix rows for `.stim`, `.dem`, result formats, gate table, targets, Pauli strings, tableaus, samplers, generated circuits, detector conversion, and detector error model analysis |
| Cover planned CLI surfaces in implementation order | Satisfied | Matrix rows for `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem` |
| Mark deferred surfaces as future work | Satisfied | Matrix future buckets and deferral reasons for diagrams, `explain_errors`, `repl`, Python bindings, JS/WASM, Crumble, Cirq, Sinter, StimFlow, ZX, lattice-surgery helpers, QASM, Quirk, and GPU acceleration |
| Define acceptance checks for implementation milestones before they begin | Satisfied | `acceptance_check` column; non-empty milestone views for M4 through M12 |
| `just oracle::matrix --check` verifies P0, P1, and Bench coverage | Satisfied | Command passed with 313 rows and non-empty counts for M4 through M12 |
| `just oracle::matrix --milestone M4` through `M12` prints non-empty task lists | Satisfied | M4: 17 rows, M5: 12 rows, M6: 29 rows, M7: 11 rows, M8: 20 rows, M9: 9 rows, M10: 29 rows, M11: 5 rows, M12: 12 rows |
| No implementation milestone below has an unnamed test or benchmark dependency | Satisfied | `just oracle::matrix --check`; `just bench::list`; milestone sections in `docs/plans/rust-stim-drop-in-rewrite.md` |
| Deferred rows include a reason and future-plan bucket | Satisfied | Matrix deferral columns validated by `ops/oracle/src/matrix.rs` |

## Milestone Audit Outcome

- M1 has no runtime feature surface; its main failure modes are missing upstream coverage, weak row metadata, or unstated deferrals.
- The matrix validator and matrix milestone views cover those risks directly.
- The 2026-06-28 GPT-5.5/xhigh milestone-audit pass initially found that this report omitted GOAL audit/review evidence.
- The report now records the audit outcome separately from implementation evidence.
- No open M1 under-specification entries remain in `docs/plans/milestone-spec-gaps.md`.

## Full Code Review Outcome

- The 2026-06-28 GPT-5.5/xhigh full-code-review pass found no blocking M1 documentation, workflow, or traceability issues.

## Verification Commands

- `just oracle::matrix --check`
- `for m in M4 M5 M6 M7 M8 M9 M10 M11 M12; do just oracle::matrix --milestone "$m" >/tmp/stab_matrix_$m.txt || exit 1; printf "%s %s\n" "$m" "$(rg -c '^-' /tmp/stab_matrix_$m.txt)"; done`
- `cargo test -p stab-oracle repository_matrix_passes_coverage_checks --quiet`
