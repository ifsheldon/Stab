# M2 Completion Report

## Milestone

M2: Oracle Corpus And Red Parity Tests.

Objective: create feature-equivalence tests before the implementation exists, so implementation milestones can turn red cases green.

## Status

Complete for the current oracle corpus and red-test workflow.

M2 owns the fixture manifest, exact-output recording workflow, structural and statistical comparator contracts, red and manifest-only fixture visibility, and source-license notes for copied or generated fixtures.
Implementation milestones M4 through M11 are responsible for turning their own rows green and for completing structural or statistical comparator implementations before rows are marked `implemented`.

## Tests And Artifacts Ported Or Created

- Added `oracle/fixtures/manifest.csv` as the source-owned oracle fixture manifest.
- Added exact-output rows for deterministic CLI and file-format cases, with expected output recorded from pinned Stim v1.16.0 when a public CLI path exists.
- Added structural comparator rows for direct Rust coverage where byte-for-byte CLI output is not the right contract.
- Added statistical comparator rows with sample counts, fixed seeds, bucket or binomial expectations, sigma tolerances, and false-positive-rate notes.
- Added explicit red rows and manifest-only rows so planned parity gaps are visible in `just oracle::list` and `just oracle::run --all`.
- Added fixture manifest validation, path safety checks, side-output placeholder validation, and source-license-note checks in `ops/oracle`.

## Implementation Areas

- `oracle/fixtures/manifest.csv` names every planned fixture, upstream source, comparator, command shape, expected status, status class, milestone, statistical plan, and source-license note.
- `oracle/fixtures/inputs/` and `oracle/fixtures/expected/` store source-owned fixture inputs and golden outputs.
- `ops/oracle/src/fixtures.rs` owns fixture parsing, validation, run filtering, red and manifest-only reporting, and record/check-clean behavior.
- `ops/oracle/src/fixtures/statistical.rs` owns statistical comparator parsing and deterministic bucket or binomial checks.
- `justfiles/oracle.just` exposes fixture listing, running, and recording.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Create an oracle fixture manifest with fixture, source, comparator, command shape, expected status, and milestone metadata | Satisfied | `oracle/fixtures/manifest.csv`; `cargo test -p stab-oracle repository_fixture_manifest_passes_validation --quiet` |
| Import or generate exact-output fixtures for deterministic parser/printer, `gen`, `convert`, deterministic sampling, `detect`, `m2d`, `.dem` parsing/printing, and CLI help cases | Satisfied | `oracle/fixtures/manifest.csv`; `oracle/fixtures/inputs/`; `oracle/fixtures/expected/`; `just oracle::record --check-clean` |
| Define structural comparator contracts | Satisfied | Structural rows in `oracle/fixtures/manifest.csv`; structural comparator logic in `ops/oracle/src/fixtures.rs`; implementation milestones mark rows `implemented` only when runnable coverage exists |
| Define statistical comparator contracts | Satisfied | Statistical rows include sample counts, fixed seeds, tolerance text, and false-positive-rate notes; `ops/oracle/src/fixtures/statistical.rs` runs the comparator |
| Mark not-yet-implemented cases as red, ignored, or manifest-only without hiding them from `just oracle::list` | Satisfied | `just oracle::list`; `just oracle::run --all` reports `RED m0-help-exact` and manifest-only M9 detector-analysis rows explicitly |
| Ensure manifest-only rows identify planned subcase groups or extraction criteria | Satisfied | `rg ",manifest-only," oracle/fixtures/manifest.csv` shows every manifest-only row with a planned extraction note |
| Add source-license notes for copied upstream tests or fixtures | Satisfied | `source_license_note` column in `oracle/fixtures/manifest.csv`; fixture manifest validator |
| `just oracle::list` prints every fixture grouped by milestone, parity mode, and status | Satisfied | Command prints M0 through M11 implemented, red, and manifest-only groups |
| `just oracle::record --check-clean` can record runnable exact-output fixtures without modifying committed fixtures | Satisfied | Command reported all recordable exact-output fixtures `CLEAN` |
| `just oracle::run --implemented-only` passes implemented smoke and parity cases | Satisfied | Command passed all implemented rows |
| `just oracle::run --all` reports unimplemented cases explicitly | Satisfied | Command completed with explicit `RED` and `MANIFEST-ONLY` rows instead of missing metadata errors |

## Milestone Audit Outcome

- M2 intentionally permits red and manifest-only rows; the acceptance bar is visibility, metadata completeness, safe paths, and runnable checks for rows marked `implemented`.
- The 2026-06-28 GPT-5.5/xhigh milestone-audit pass initially found that this report omitted GOAL audit/review evidence.
- The report now records the audit outcome separately from implementation evidence.
- No open M2 under-specification entries remain in `docs/plans/milestone-spec-gaps.md`.
- Remaining red or manifest-only rows are owned by later milestone scopes or future detector-analysis work, not by M2 itself.

## Full Code Review Outcome

- The 2026-06-28 GPT-5.5/xhigh full-code-review pass found no blocking M2 documentation, oracle-workflow, or traceability issues.

## Verification Commands

- `just oracle::list`
- `just oracle::record --check-clean`
- `rg ",manifest-only," oracle/fixtures/manifest.csv`
- `just oracle::run --implemented-only`
- `just oracle::run --all`
- `cargo test -p stab-oracle repository_fixture_manifest_passes_validation --quiet`
