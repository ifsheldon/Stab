# PFM-B0 Blocker Ledger Progress Report

Date: 2026-07-10

Status: Complete.

## Objective

Freeze all eight non-deferred blocker families into a source-owned, machine-checkable, subcase-level ledger before shared semantic implementation begins.
PFM-B0 adds no Stim behavior and changes no benchmark gate.

## Implementation

- Added `docs/plans/blocker-closure-ledger.json` with schema version 1 and frozen Stim v1.16.0 provenance.
- Recorded 124 owned cases across PFM-B1 through PFM-B5.
- Split the broad Python `test_inv_circuit`, C++ `missing_detectors.circuit`, and C++ `circuit_flow_generators.various` sources into stable case ids.
- Recorded each case's public or internal surface, upstream source and subcase, comparator, current status, test selector, oracle disposition, benchmark disposition and comparability class, and resource contract.
- Distinguished exact GTest cases, exact pytest cases, planned cross-cutting source test families, and implementation symbols. Exact tests and symbols are checked against tracked source text, and every planned test family names one or more exact validated upstream anchors; a test-family record cannot be used as implemented or evidence-close proof.
- Recorded 17 additional promoted oracle rows required to support the three evidence-close blocker decisions instead of treating one primary row as the whole closure argument, with exact parity, comparator, argv, and upstream-source signatures included in the semantic inventory.
- Recorded five additional detector-utility benchmark rows required by the evidence-close decisions so their `contract-only` runner, `non-primary-report-only` threshold class, and `report-only` comparability class cannot drift independently.
- Added `stab-oracle blockers` and the `just oracle::blockers` operational recipe.
- Added strict validation for all eight required owners, a canonical SHA-256 semantic inventory covering provenance, status, tests, oracle and benchmark evidence, statistical plans, and resource contracts, plus minimum owned-case floors, duplicate ids, pinned and tracked regular upstream files, planned versus existing test-state honesty, implemented oracle rows, typed oracle and benchmark runner classifications, stable regular ledger and manifest inputs, and bounded input and display fields.
- Made benchmark runner, threshold, and comparability classifications separately source-owned so a row cannot preserve its runner while silently changing comparison semantics.
- Added `just oracle::blockers --list` for case-level inspection.
- Added `just oracle::blockers --check-selectors`, which resolves allowlisted Cargo selectors through the Rust test harness, rejects option-shaped filters and zero-test matches, places filters after Cargo's argument separator, and uses the oracle harness's timed, output-bounded process runner.
- Require pre-open and post-open regular-file checks plus Unix device/inode identity for ledger, manifest, and upstream evidence inputs; the validator fails closed on targets without a stable identity implementation.
- Keep the selector process deadline active through child exit and stdin, stdout, and stderr completion; timeout kills and reaps the Unix process group even when the direct child exits before a descendant that retains an output pipe.

## Ledger Summary

| Blocker | Decision | Cases | Planned | Implemented | Evidence close | Shared selectors | Supporting oracles | Supporting benchmarks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| PFM2 QEC transforms | Implement | 19 | 7 | 12 | 0 | 4 | 0 | 0 |
| PFM3 analyzer sweep | Evidence close | 1 | 0 | 0 | 1 | 0 | 1 | 0 |
| PFM3 gate execution | Implement | 12 | 12 | 0 | 0 | 0 | 0 | 0 |
| PFM4 DEM traversal | Implement | 7 | 7 | 0 | 0 | 0 | 0 | 0 |
| PFM5 detecting regions | Evidence close | 2 | 0 | 0 | 2 | 0 | 10 | 4 |
| PFM5 missing detectors | Evidence close | 14 | 0 | 0 | 14 | 11 | 6 | 1 |
| PFM5 flow engine | Implement | 33 | 1 | 32 | 0 | 28 | 0 | 0 |
| PFM6 analyzer and search | Implement | 36 | 14 | 22 | 0 | 13 | 0 | 0 |

The 41 planned cases are explicit future implementation work.
The 66 implemented cases retain existing semantic evidence, and the 17 evidence-close-designated cases belong to the three families where the active pinned behavior is already implemented.
The ledger also exposes 56 cases that currently share a broad Rust selector, including 11 missing-detector evidence-close cases; their stable case ids freeze the inventory, but PFM-B1, PFM-B4, and PFM-B5 must replace shared selectors with independently selectable tests before their respective acceptance criteria can close.
Each statistical plan uses 100,000 shots, a frozen per-case seed, a six-sigma binomial standard-error term, a 0.01 absolute probability floor, named output buckets, and a familywise false-positive budget of 0.000001; the owner milestone must verify that budget with an exact binomial-tail calculation before promotion.

## Tests

`ops/oracle/src/blocker_ledger/tests.rs` covers:

- Validation of the committed source ledger.
- Unknown schema versions.
- Missing required blockers.
- Deleted owned cases below a blocker-specific floor.
- Duplicate case ids.
- Replacement of a frozen owned case with a different case id.
- Planned rows that falsely claim existing test evidence.
- Evidence-close rows that point only to planned oracle evidence.
- Stale existing oracle rows.
- Stale existing benchmark rows.
- Unsafe upstream traversal paths.
- Missing benchmark comparability classifications.
- Missing comparator, benchmark disposition, concrete resource contract, and executable-selector fields.
- Non-implemented oracle rows and mismatched oracle evidence classifications.
- Missing or changed supporting-oracle sets for evidence-close blockers.
- Missing or changed supporting-benchmark sets or runner-class drift for evidence-close blockers.
- Unanchored test-family aggregation or any test-family record promoted as completion evidence.
- Statistical comparators without frozen shots, seeds, sigma multipliers, absolute probability floors, bucket definitions, and familywise false-positive budgets.
- Benchmark runner-class drift.
- Non-allowlisted Cargo arguments, option-shaped filters, misplaced harness filters, and zero-trust selector shapes.
- Oracle direct-versus-Rust-proxy runner-class drift.
- Primary and supporting oracle comparator, argv, parity-mode, and upstream-source signature drift.
- Symlinked ledger input, nonblocking FIFO rejection, and unstable regular-file identity.
- Untracked upstream source files, implicit or missing source-symbol provenance, and terminal control characters in displayed text.
- Actual ledger-byte caps during bounded reads, independent of metadata preflight.
- Process timeouts where the direct child remains alive, the direct child exits before a descendant holding output pipes, or a child blocks the stdin writer.

## Benchmarks

PFM-B0 adds no timing workload and changes no threshold.
Every ledger case nevertheless has an existing benchmark row, a planned benchmark row with comparability class, or a concrete no-benchmark rationale.

## Acceptance Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| All eight open spec-gap entries have an owner | Satisfied | `EXPECTED_BLOCKERS` and `just oracle::blockers` |
| One stable ledger record exists per owned subcase | Satisfied | 124 semantically frozen case records in `docs/plans/blocker-closure-ledger.json`, including separate loop-carried, period-8, and period-127 `ErrorAnalyzer.loop_folding` records |
| Multi-example broad tests have stable subcase inventory | Satisfied | Stable PFM2, PFM5 missing-detector, and PFM5 flow case ids; 56 shared selectors remain explicitly visible for owner-milestone evidence splitting |
| Existing work is distinguished from planned work | Satisfied | `status` plus test, oracle, and benchmark evidence states |
| Comparators and resource contracts are mandatory | Satisfied | `BlockerCase` schema and validator; statistical rows additionally require reproducible typed statistical plans |
| Oracle and benchmark metadata remain source-owned | Satisfied | Existing ids are checked against both CSV manifests |
| No timing run is required | Satisfied | Metadata-only milestone with no gate changes |
| Milestone-audit closure | Satisfied | Final audit found no remaining completion blocker or specification loophole after the end-to-end process-timeout fix |
| Full-code-review closure | Satisfied | GPT-5.6/max review findings for benchmark comparability ownership, oracle evidence signatures, and descendant process termination were fixed and re-reviewed |

## Verification

Completed during implementation:

- `cargo fmt --all`
- `cargo test -p stab-oracle blocker_ledger --quiet`
- `cargo run -q -p stab-oracle -- blockers`
- `cargo run -q -p stab-oracle -- blockers --check-selectors`
- `cargo clippy -p stab-oracle --all-targets -- -D warnings`
- `cargo test -p stab-oracle --quiet`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just oracle::run --implemented-only`
- `just bench::list`
- `just oracle::blockers --list`
- `just bench::smoke`
- `just maintenance::pre-commit`

The final milestone audit and full code review reported no remaining completion blockers.
