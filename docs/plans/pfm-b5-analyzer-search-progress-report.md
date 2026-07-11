# PFM-B5 Generic Analyzer And Search Progress Report

## Status

PFM-B5 implementation, executable evidence, and milestone audit are complete for the selected Rust and CLI scope at `872e3a3`.
The milestone owns 36 independently selected ledger cases: eight analyzer cases, seven graphlike cases, eight hypergraph cases, ten shortest or weighted WCNF cases, two sparse reverse-tracker cases, and one matched-error value-object case.
GPT-5.6/max full-code-review closure remains required before this report may be treated as the final PFM-B5 completion record.

## Scope Result

The production analyzer no longer recognizes fixture text, exact instruction sequences, or the previously promoted periods 8 and 127.
One reverse-state engine now discovers transients and recurrences from canonical sparse analyzer boundary state, validates the candidate period, emits compact repeated DEM bodies, and composes detector, measurement, coordinate, observable, gauge, and analyzer-mode state with checked arithmetic.
The old bounded analyzer remains only for the explicitly preflighted instruction families `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`, which the generic reverse-fold implementation does not yet summarize.

Graphlike and hypergraph construction allocate nodes only for detector IDs touched by nonzero error mechanisms.
Graphlike search and SAT/WCNF encoding compress touched detector and observable IDs into stable sorted slots instead of allocating through the maximum sparse ID.
The generated d11/r1000 graphlike workload now passes traversal admission instead of failing because repeated annotations consumed an unrelated expanded-instruction budget.

## Executable Case Matrix

Analyzer cases:

- Giant loop-carried observable recurrence.
- Period-8 and period-127 observable recurrences discovered without period-specific production code.
- Nested repeat folding.
- Generated repetition-code compact folding.
- Multi-round gauge-state folding.
- Nested coordinate and detector-shift composition.
- Cross-iteration dependency rejection.

Search and encoding cases:

- Graphlike no-error, distance one, distance two, distance three, generated surface code, generated repetition code, and sparse high-observable behavior.
- Hypergraph no-error, distance one, distance two, distance three, bounded four-detector mechanisms, generated surface code, generated repetition code, and sparse high-observable behavior.
- Exact shortest and weighted WDIMACS output for empty, no-error, detector-only, observable-only, no-target, detector-plus-observable, ordinary-probability, large-probability, and half-probability models.
- Sparse reverse tracker unitary-repeat and shifted-repeat state equivalence.
- Active matched-error canonicalization and value validation.

The source-owned contract is `docs/plans/blocker-closure-ledger.json`.
`just oracle::blockers --check-selectors` reports all 36 PFM-B5 cases implemented with no shared selectors, seven supporting oracle rows, and eleven supporting benchmark rows.

## Tests And Oracle Evidence

`crates/stab-core/tests/dem_analyzer_pfm_b5.rs` adds exact nested, coordinate, gauge, and generated repetition-code cases plus sixteen seeded folded-versus-unrolled Clifford, measurement, detector, and observable cases and a nested coordinate differential case.
Existing exact giant-repeat tests continue to prove loop-carried, period-8, and period-127 output against pinned Stim v1.16.0.

`crates/stab-core/tests/dem_search_pfm_b5.rs` contains seven independently selectable graphlike cases, eight independently selectable hypergraph cases, and ten independently selectable exact WCNF cases.
The graphlike and hypergraph distance cases assert exact canonical DEM text where ordering is contractual; generated QEC cases use minimum-distance and canonical target-signature invariants where equal-length paths are tie-sensitive.

Pinned CLI oracle fixtures were added for nested, coordinate, and gauge loop folding.
The repetition-code analyzer and finite graphlike, hypergraph, and WCNF corpora are represented by independently resolving Cargo-test proxy rows whose selected tests contain the semantic or exact assertions.
The Cargo-test proxy rows remain structurally classified because the oracle runner validates test execution rather than capturing the values asserted inside a Rust test.

## Resource Contracts

Analyzer recurrence discovery is limited to 1,000,000 candidate steps and uses memory proportional to canonical boundary state, one loop body, and emitted compact DEM output.
The bounded fallback remains subject to its existing repeat-count, repeat-iteration, and expanded-instruction limits.

Graphlike and hypergraph traversal count expanded nonzero error mechanisms separately from annotations and reject more than 5,000,000 such mechanisms in production.
Graphlike and hypergraph construction independently cap effective touched detector nodes at 1,000,000.
Repeat-iteration and repeat-nesting guards remain separate from both limits.
SAT/WCNF generation retains its explicit emitted-clause and repeat-expansion limits because the returned encoding is inherently materialized.

The new traversal tests prove that 10,001 repeated annotations plus one error count as one search mechanism, while 10,001 shifted nonzero errors hit the test-only search-mechanism boundary with a search-specific error message.
A parameterized touched-detector test crosses the effective-node boundary without allocating one million nodes, and SAT unit tests prove that the production detector and observable target caps are independently inclusive at the boundary and fail separately above it.

## Benchmark Evidence

The final source-owned artifacts are:

- Baseline: `target/benchmarks/pfm-b5-completion-baseline-final2/baseline.json`.
- Compare: `target/benchmarks/pfm-b5-completion-compare-final2/compare.json`.
- Stab commit: `0c6deb2654426e1ec2d1d0489f361bdb9b4b2ed0`.
- Frozen Stim commit: `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, tag `v1.16.0`.
- Worktree state: `local_modifications=false`.
- Method: release profile, warmup, three measurement runs, allocation tracking, and required profiler notes.

| Row | Representative Stab time | Stim time | Ratio | Peak live allocation | Disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| `pfm-b5-analyzer-cycle-folding` | 6.720 us to 490.177 us | No faithful aggregate filter | None | 94,336 B | Report-only |
| `pfm-b5-analyzer-generated-qec` | 46.288 us and 13.803 ms | No faithful aggregate filter | None | 6,430,472 B | Report-only |
| `pfm-b5-graphlike-search-direct-dem` | 799.937 us | No faithful direct-model filter | None | 615,896 B | Report-only |
| `pfm-b5-graphlike-generated-d25` | 184.066 ms | 31 ms | 5.938x | 12,437,520 B | Direct match, report-only |
| `pfm-b5-graphlike-generated-d11-r1000` | 1.417 s | 270 ms | 5.247x | 92,643,408 B | Direct match, report-only |
| `pfm-b5-hypergraph-search-direct-dem` | 53.520 us | No faithful direct-model filter | None | 101,184 B | Report-only |
| `pfm-b5-hypergraph-search-generated-qec` | 64.110 ms | No faithful filter | None | 12,868,696 B | Report-only |
| `pfm-b5-wcnf-direct-dem` | 368.161 us and 464.528 us | No faithful filter | None | 521,984 B | Report-only |
| `pfm-b5-wcnf-generated-qec` | 3.230 ms and 3.674 ms | No faithful filter | None | 4,970,256 B | Report-only |

The analyzer diagnostics prove that the transient, period-8, period-127, nested, gauge, and coordinate workloads all use the generic reverse-fold path with no bounded fallback.
The gauge case represents `10^15` repeat iterations and arithmetically skips `999,999,999,999,996` entered-loop iterations.
For nested loops, `represented_repeat_iterations` recursively counts all source-represented inner work, while `folded_repeat_iterations` counts arithmetic skips at the loop levels actually entered by the analyzer and must not be interpreted as the total represented nested work.

The two faithful graphlike rows do not meet the 1.25x performance gate and were not added to `benchmarks/m12-primary-thresholds.json`.
Their source-owned profiler notes record the allocation evidence, the host's `perf_event_paranoid=4` sampling limitation, and the required compact interned-state frontier work.
All other new rows remain report-only because no pinned Stim filter measures a faithful equivalent workload.

## Verification

The implementation and evidence changes passed:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle --quiet
just oracle::record --check-clean
just oracle::blockers --check-selectors
just bench::smoke
just bench::baseline --only PF6 --out target/benchmarks/pfm-b5-completion-baseline-final2
cargo run -q -p stab-bench --profile release --features count-allocations -- compare --only PF6 --baseline target/benchmarks/pfm-b5-completion-baseline-final2/baseline.json --report target/benchmarks/pfm-b5-completion-compare-final2 --track-allocations --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/pfm-b5
```

## Milestone Audit

Status: Complete.

The audit found one completion-blocking evidence gap: the ledger documented independent effective detector-node and SAT target-count caps, but the tests proved sparse-ID compression and the nonzero-mechanism cap without directly crossing those effective-target boundaries.
Commit `872e3a3 test(core): close PFM-B5 resource boundaries` adds a parameterized shared search-target boundary test and independent inclusive SAT detector and observable boundary tests.
No production defect or newly revealed milestone under-specification remained after that fix, so no new entry was added to `docs/plans/milestone-spec-gaps.md`.

| Requirement | Status | Evidence |
| --- | --- | --- |
| Generic analyzer recurrence without fixture or hard-coded period dispatch | Satisfied | `crates/stab-core/src/dem/analyze/reverse_fold.rs`; exact and generated analyzer tests |
| Nested, coordinate, gauge, generated QEC, and cross-iteration behavior | Satisfied | `crates/stab-core/tests/dem_analyzer_pfm_b5.rs`; ledger analyzer cases |
| Finite graphlike and hypergraph closure corpora | Satisfied | `crates/stab-core/tests/dem_search_pfm_b5.rs`; seven graphlike and eight hypergraph cases |
| Exact shortest and weighted WCNF corpus | Satisfied | Ten independently selected `pfm_b5_wcnf_*` tests |
| Sparse IDs and distinct traversal or search limits | Satisfied | Sparse resource tests, traversal mechanism tests, and `872e3a3` boundary tests |
| Source-owned oracle and benchmark evidence | Satisfied | Seven supporting oracle rows, eleven benchmark rows, and clean final2 artifacts |
| Honest 1.25x gate disposition | Satisfied | Direct-match ratios 5.938x and 5.247x remain report-only with profiler notes |
| Deferred provenance remains excluded | Satisfied | Checklist, ledger, and this report name full ErrorMatcher provenance as deferred |

## Remaining Work Outside PFM-B5

The selected PFM-B5 semantic scope has no intentionally unimplemented child case.
Full ErrorMatcher stack-frame, heralded, and repeat-contained provenance plus `stim explain_errors` remain intentionally deferred.
The graphlike direct-match slowdown is an optimization backlog item and an explicit reason the rows remain outside the primary gate; it does not invalidate semantic closure.
PFM-B2 still owns the generated exhaustive gate-by-surface semantic matrix, and PFM-B6 still owns final audit and status rollup.
