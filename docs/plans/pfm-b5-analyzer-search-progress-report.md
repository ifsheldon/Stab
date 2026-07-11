# PFM-B5 Generic Analyzer And Search Progress Report

## Status

PFM-B5 implementation, executable evidence, original milestone-audit remediation, and earlier full-code-review remediation were complete through `37cf586`.
A later final review found a supported-unitary nested-probe bypass, unbounded graph-construction work and payloads, allocation-heavy graphlike state comparison, weak generated-result membership checks, stale fallback metadata, and an overstated SAT output-bound claim.
Production remediation is committed in `a7173fe`, and the expanded executable evidence is committed in `23b0d72`.
The milestone now owns 52 independently selected ledger cases: fifteen analyzer cases, ten graphlike cases, eleven hypergraph cases, twelve shortest or weighted WCNF cases, two sparse reverse-tracker cases, one shared search-traversal resource case, and one matched-error value-object case.
Fresh committed-HEAD benchmark evidence is recorded at `93b80da`; a new milestone-audit pass and a final full-code-review pass remain required before this report may be treated as the PFM-B5 completion record.

## Scope Result

The production analyzer no longer recognizes fixture text, exact instruction sequences, or the previously promoted periods 8 and 127.
One shared shifted-recurrence engine now discovers transients and recurrences from canonical sparse boundary state for both the analyzer and sparse reverse-tracker repeat folding, while analyzer-specific code validates and emits compact repeated DEM bodies and composes detector, measurement, coordinate, observable, gauge, and analyzer-mode state with checked arithmetic.
Folded `MPAD(p)` instructions record their measurement-error mechanism before reverse tracker consumption, matching the ordinary measurement path.
The old bounded analyzer remains only for the explicitly preflighted instruction families `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`, which the generic reverse-fold implementation does not yet summarize.

Graphlike and hypergraph construction allocate nodes only for detector IDs touched by nonzero error mechanisms.
Graphlike search and SAT/WCNF encoding compress touched detector and observable IDs into stable sorted slots instead of allocating through the maximum sparse ID, which is an intentional resource-hardening semantic deviation for sparse WCNF IDs rather than a byte-exact Stim claim.
Graphlike and hypergraph construction use indexed edge deduplication and independently bound unique edges and persistent edge, observable-mask, and adjacency payloads before installation; hypergraph adjacency interns each edge once instead of cloning its detector set into every incident node.
Graphlike and hypergraph searches enforce independent state, transition, per-state payload, and aggregate stored-payload budgets, and graphlike state ordering borrows observable masks instead of cloning them during tree comparison.
Search traversal rejects both oversized per-error target lists and excessive aggregate source-target work before normalization or shifted-target allocation, and zero-probability source errors retain exact Stim-compatible failure diagnostics instead of being reported as an absent error declaration.
SAT/WCNF encoding returns the canonical trivial UNSAT problem before flattening models with no observables or no source error declarations, then preflights mechanisms, target occurrences, variables, stored clauses, and literals before clause allocation for nontrivial models; the conservative output-byte guard remains redundant defense in depth behind those tighter materialization limits.
Finite selected WCNF output matches Stim's stored-clause header convention even when quantization suppresses a zero-weight output line.
The generated d11/r1000 graphlike workload now passes traversal admission instead of failing because repeated annotations consumed an unrelated expanded-instruction budget.

## Executable Case Matrix

Analyzer cases:

- Giant loop-carried observable recurrence.
- Period-8 and period-127 observable recurrences discovered without period-specific production code.
- Nested repeat folding.
- Generated repetition-code compact folding.
- Multi-round gauge-state folding.
- Nested billion-round gauge-state folding with transient gauge output consumed during recurrence probes.
- Nested coordinate and detector-shift composition.
- Saturating observational diagnostics across multiple maximum-count repeats.
- The local decomposition boundary accepts sixteen detector symptoms and rejects seventeen.
- Folded noisy `MPAD` parity.
- One cumulative recurrence-probe work budget across nested circuit entries and instructions, including nested repeats eligible for the normal supported-unitary fast path.
- Cross-iteration dependency rejection.

Search and encoding cases:

- Graphlike no-error, distance one, distance two, distance three, generated surface code, generated repetition code, and sparse high-observable behavior.
- Hypergraph no-error, distance one, distance two, distance three, bounded four-detector mechanisms, generated surface code, generated repetition code, and sparse high-observable behavior.
- Exact Stim-compatible zero-probability diagnostics, source-membership checks for generated returned mechanisms, independently bounded variable-sized graphlike and hypergraph search states, indexed graph construction, unique-edge and persistent-payload admission, and aggregate source-target traversal work.
- Exact shortest and weighted WDIMACS output for twelve selected finite empty, no-error, detector-only, large detector-only, observable-only, no-target, detector-plus-observable, ordinary-probability, large-probability, half-probability, and low-quantization header cases; sparse-ID and large folded-repeat compression retain documented semantic hardening.
- Sparse reverse tracker unitary-repeat and shifted-repeat state equivalence.
- Active matched-error canonicalization and value validation.

The source-owned contract is `docs/plans/blocker-closure-ledger.json`.
`just oracle::blockers --check-selectors` reports all 52 PFM-B5 cases implemented with no shared selectors, fifteen supporting oracle rows, and eleven supporting benchmark rows.

## Tests And Oracle Evidence

`crates/stab-core/tests/dem_analyzer_pfm_b5.rs` adds exact nested, coordinate, gauge, nested-gauge, diagnostic-saturation, local-decomposition-boundary, folded noisy `MPAD`, and generated repetition-code cases plus sixteen seeded folded-versus-unrolled Clifford, measurement, detector, and observable cases and a nested coordinate differential case.
Analyzer unit regressions independently cross the cumulative nested-probe work limit for ordinary nested work and for a supported-unitary billion-repeat body that previously bypassed the budget; fallback integration tests enter a genuinely unsupported instruction path before independently crossing repeat-count, aggregate-repeat-iteration, and expanded-instruction limits.
Existing exact giant-repeat tests continue to prove loop-carried, period-8, and period-127 output against pinned Stim v1.16.0.

`crates/stab-core/tests/dem_search_pfm_b5.rs` contains the finite graphlike and hypergraph semantic corpora, separate zero-probability diagnostic cases, and twelve independently selectable exact WCNF cases.
Graphlike and hypergraph unit regressions independently cross per-state and aggregate stored-state limits, unique graph-edge limits, persistent graph-payload limits, and the shared aggregate target-work limit before the corresponding persistent allocation.
The graphlike and hypergraph distance cases assert exact canonical DEM text where ordering is contractual; generated QEC cases use minimum-distance and canonical target-signature invariants where equal-length paths are tie-sensitive and additionally require every returned mechanism to occur in the source DEM.

Pinned CLI oracle fixtures cover nested, coordinate, gauge, nested-gauge, diagnostic-saturation, sixteen-symptom local-decomposition, and folded noisy `MPAD` behavior.
Direct CLI evidence signatures bind fixture paths and SHA-256 digests while still executing live Stim-versus-Stab comparison, and every implemented PFM-B5 selector resolves one full test name with `--exact`.
The repetition-code analyzer and finite graphlike, hypergraph, and WCNF corpora are represented by independently resolving Cargo-test proxy rows whose selected tests contain the semantic or exact assertions.
The Cargo-test proxy rows remain structurally classified because the oracle runner validates test execution rather than capturing the values asserted inside a Rust test.

## Resource Contracts

Analyzer recurrence discovery is limited to 1,000,000 cumulative work units across nested circuit entries and probed instructions, including supported-unitary nested repeats, and uses memory proportional to canonical boundary state, one loop body, and emitted compact DEM output.
Analyzer recurrence probes consume transient gauge output at the same instruction boundary as real analysis, and all diagnostic counters saturate so telemetry cannot reject a valid circuit.
The bounded fallback remains subject to its existing repeat-count, repeat-iteration, and expanded-instruction limits.

Graphlike and hypergraph traversal count expanded nonzero error mechanisms separately from annotations, reject more than 5,000,000 such mechanisms in production, cap each mechanism at 65,536 source target occurrences, and cap aggregate source-target work at 20,000,000 occurrences.
Graphlike and hypergraph construction independently cap effective touched detector nodes at 1,000,000.
Graphlike and hypergraph construction cap unique edges at 5,000,000 and persistent edge-payload, compact-index, and adjacency terms at 20,000,000; graphlike and hypergraph edge lookup use collision-checked randomized arena hash indexes instead of linear scans or duplicate edge payloads. The five-million edge ceiling preserves the selected generated d11/r1000 workload that exposed the original one-million ceiling as too strict.
Both searches cap unique states at 1,000,000 and attempted transitions at 20,000,000 in production.
Each search state is capped at 65,536 detector and observable terms, and aggregate persisted map, predecessor, and queue copies are capped at 5,000,000 terms.
Hypergraph construction caps an explored edge at 4,096 detector symptoms and total edge incidences at 5,000,000 while storing each unique edge once in an arena.
Repeat-iteration and repeat-nesting guards remain separate from both limits.
SAT/WCNF generation caps 250,000 materialized mechanisms, 500,000 target occurrences, 500,000 variables, 500,000 stored clauses, and 1,500,000 clause literals before allocation; repeat-expansion limits remain separate because the returned encoding is inherently materialized. A 128 MiB conservative output guard remains as redundant defense in depth and is not claimed as independently reachable behind the stricter clause and literal caps.

The traversal tests prove that 10,001 repeated annotations plus one error count as one search mechanism, while 10,001 shifted nonzero errors hit the test-only search-mechanism boundary with a search-specific error message.
A parameterized touched-detector test crosses the effective-node boundary without allocating one million nodes; graphlike and hypergraph tests independently cross per-state, aggregate state-payload, unique-edge, and graph-payload budgets; a shared traversal test crosses aggregate source-target work; and SAT tests prove the total target-occurrence boundary plus early trivial UNSAT handling for a large detector-only shifted repeat.

## Benchmark Evidence

The current clean post-final-review source-owned artifacts are:

- Baseline: `target/benchmarks/pfm-b5-final-review-v3-baseline/baseline.json`.
- Compare: `target/benchmarks/pfm-b5-final-review-v3-compare/compare.json`.
- Stab commit: `93b80dafcf50282088d96c68604f84bf0eed94e1`.
- Frozen Stim commit: `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`, tag `v1.16.0`.
- Worktree state: `local_modifications=false`.
- Method: release profile, warmup, three measurement runs, allocation tracking, and required profiler notes.

| Row | Representative Stab time | Stim time | Ratio | Peak live allocation | Disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| `pfm-b5-analyzer-cycle-folding` | 6.832 us to 497.488 us | No faithful aggregate filter | None | 94,336 B | Report-only |
| `pfm-b5-analyzer-generated-qec` | 46.624 us and 14.035 ms | No faithful aggregate filter | None | 6,430,472 B | Report-only |
| `pfm-b5-graphlike-search-direct-dem` | 345.312 us | No faithful direct-model filter | None | 663,000 B | Report-only |
| `pfm-b5-graphlike-generated-d25` | 146.499 ms | 31 ms | 4.726x | 15,461,688 B | Direct match, report-only |
| `pfm-b5-graphlike-generated-d11-r1000` | 1.096 s | 260 ms | 4.214x | 115,199,520 B | Direct match, report-only |
| `pfm-b5-hypergraph-search-direct-dem` | 56.272 us | No faithful direct-model filter | None | 78,488 B | Report-only |
| `pfm-b5-hypergraph-search-generated-qec` | 50.898 ms | No faithful filter | None | 12,440,968 B | Report-only |
| `pfm-b5-wcnf-direct-dem` | 392.817 us and 442.417 us | No faithful filter | None | 453,518 B | Report-only |
| `pfm-b5-wcnf-generated-qec` | 3.385 ms and 3.613 ms | No faithful filter | None | 3,844,106 B | Report-only |

The analyzer diagnostics prove that the transient, period-8, period-127, nested, gauge, and coordinate workloads all use the generic reverse-fold path with no bounded fallback.
The gauge case represents `10^15` repeat iterations and arithmetically skips `999,999,999,999,996` entered-loop iterations.
For nested loops, `represented_repeat_iterations` recursively counts all source-represented inner work, while `folded_repeat_iterations` counts arithmetic skips at the loop levels actually entered by the analyzer and must not be interpreted as the total represented nested work.

The two faithful graphlike rows do not meet the 1.25x performance gate and were not added to `benchmarks/m12-primary-thresholds.json`.
Collision-checked edge-arena indexing improves the clean ratios from 5.960x to 4.726x for d25/r25 and from 5.490x to 4.214x for d11/r1000 while bounding construction work. Peak live allocation rises from the pre-index 12,437,520 bytes to 15,461,688 bytes for d25 and from 92,643,408 bytes to 115,199,520 bytes for d11/r1000 because the compact hash index adds one arena position per edge; the rejected duplicate-payload prototype reached 31,165,504 and 214,962,720 bytes respectively and was not retained.
The source-owned profiler notes record this tradeoff, the host's `perf_event_paranoid=4` sampling limitation, and the remaining compact interned-state frontier work.
All other new rows remain report-only because no pinned Stim filter measures a faithful equivalent workload.

## Verification

The first-review implementation and evidence at `15b55cc` passed:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle --quiet
just oracle::record --check-clean
just oracle::blockers --check-selectors
just bench::smoke
just bench::baseline --only PF6 --out target/benchmarks/pfm-b5-review-baseline
cargo run -q -p stab-bench --profile release --features count-allocations -- compare --only PF6 --baseline target/benchmarks/pfm-b5-review-baseline/baseline.json --report target/benchmarks/pfm-b5-review-compare --track-allocations --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/pfm-b5
```

The earlier second-review remediation passed:

```sh
cargo fmt --all --check
cargo clippy -p stab-core --all-targets --features ops-contracts -- -D warnings
cargo test -p stab-core --features ops-contracts --quiet
cargo test -p stab-oracle blocker_ledger --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::blockers --check-selectors
just oracle::run --milestone PF6 --exact
```

The earlier second-review benchmark refresh additionally passed:

```sh
just bench::baseline --only PF6 --out target/benchmarks/pfm-b5-second-review-baseline
cargo run -q -p stab-bench --profile release --features count-allocations -- compare --only PF6 --baseline target/benchmarks/pfm-b5-second-review-baseline/baseline.json --report target/benchmarks/pfm-b5-second-review-compare --track-allocations --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/pfm-b5
```

The post-final-review benchmark refresh passed from clean `HEAD=93b80dafcf50282088d96c68604f84bf0eed94e1`:

```sh
just bench::baseline --only PF6 --out target/benchmarks/pfm-b5-final-review-v3-baseline
cargo run -q -p stab-bench --profile release --features count-allocations -- compare --only PF6 --baseline target/benchmarks/pfm-b5-final-review-v3-baseline/baseline.json --report target/benchmarks/pfm-b5-final-review-v3-compare --track-allocations --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/pfm-b5
```

Post-final-review remediation has passed:

```sh
cargo fmt --all --check
cargo clippy -p stab-core --all-targets --features ops-contracts -- -D warnings
cargo test -p stab-core --features ops-contracts --quiet
cargo test -p stab-oracle blocker_ledger --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::blockers --check-selectors
just oracle::run --milestone PF6 --exact
```

Final workspace verification remains pending.

## Audit And Review Status

Status: Reopened after a later final full-code review found additional defects.

The first audit found one completion-blocking evidence gap: the ledger documented independent effective detector-node and SAT target-count caps, but the tests proved sparse-ID compression and the nonzero-mechanism cap without directly crossing those boundaries.
Commit `872e3a3 test(core): close PFM-B5 resource boundaries` added a valid parameterized touched-detector boundary test, but its SAT detector and observable tests exercised a helper whose production cap was unreachable behind the stricter total target-occurrence limit. The second review removed that shadow cap and replaced the claim with the one executable total-occurrence contract.
The later GPT-5.6/max full-code review found production defects and evidence gaps that the original audit missed: nested gauge probe output could prevent recurrence, sixteen detector symptoms were rejected, diagnostic counters could alter semantics, search frontiers were unbounded, hypergraph adjacency could grow quadratically, SAT materialization lacked independent limits, low-quantization WCNF headers diverged from Stim, direct oracle fixtures were not content-bound, PFM-B5 selectors were not required to be exact, and generated WCNF rates used the wrong work unit.
Commits `642ff63`, `83ed962`, `8908e7f`, `f0c6a83`, `07bb198`, and `15b55cc` fix those findings without adding a new under-specification entry; the remaining documented fallback and sparse WCNF deviations were already explicit scope or resource decisions.
The next required full-code-review pass found that folded noisy `MPAD` errors were dropped, nested no-recurrence probes could evade the outer step budget, count-only search budgets did not bound variable-sized state payloads, trivial detector-only SAT models flattened large repeats before returning UNSAT, zero-probability error diagnostics diverged from Stim, and several exact or resource claims were broader than their selected evidence.
Commits `d1d6554`, `433252c`, and `d3ffc5f` fix those findings, split overclaimed ledger rows, add direct folded-`MPAD` and structural resource evidence, and expand PFM-B5 from 39 to 48 independently selected cases.
A later final review found that nested supported-unitary repeats still bypassed analyzer-probe admission, graph construction could spend quadratic time and retain unbounded edge payload before search-state admission, graphlike comparisons cloned observable masks, generated search comparators did not prove source membership, zero-probability diagnostics were asserted only by substring, the generated coordinate row still claimed fallback, and the SAT output-byte limit was documented as independent despite being unreachable behind stricter clause and literal caps.
Commit `a7173fe` shares shifted-recurrence discovery, routes analyzer probes around the normal unitary fast path, adds aggregate traversal and graph-construction budgets, indexes edge lookup, removes graphlike comparison clones and duplicate map traversals, and strengthens exact and generated search tests. Commit `23b0d72` expands PFM-B5 from 48 to 52 independent cases, freezes fifteen supporting oracle rows, and proves the generated coordinate case uses generic reverse folding without fallback.
Fresh benchmark evidence is complete at clean `HEAD=93b80dafcf50282088d96c68604f84bf0eed94e1`; milestone audit and final full-code review have not yet been completed against this synchronized implementation and evidence, so PFM-B5 remains open.

| Requirement | Status | Evidence |
| --- | --- | --- |
| Generic analyzer recurrence without fixture or hard-coded period dispatch | Satisfied | `crates/stab-core/src/dem/analyze/reverse_fold.rs`; exact and generated analyzer tests |
| Nested, coordinate, gauge, folded `MPAD`, generated QEC, supported-unitary probe admission, and cross-iteration behavior | Satisfied | `crates/stab-core/tests/dem_analyzer_pfm_b5.rs`; analyzer resource regressions; fifteen ledger analyzer cases |
| Finite graphlike and hypergraph closure corpora | Satisfied | Semantic corpus, exact zero-probability diagnostics, source-membership checks, and state or construction budgets across ten graphlike and eleven hypergraph ledger cases plus one shared traversal case |
| Exact shortest and weighted WCNF corpus | Satisfied | Twelve independently selected `pfm_b5_wcnf_*` cases |
| Sparse IDs and distinct traversal, graph-construction, or search limits | Satisfied | Sparse resource tests; per-error and aggregate traversal tests; unique-edge, graph-payload, state, transition, state-payload, edge-arena, and SAT preflight tests |
| Source-owned oracle evidence | Satisfied | Content-bound direct rows, 52 exact selectors, fifteen supporting oracle rows, and ten direct exact PF6 rows |
| Fresh source-owned benchmark evidence | Satisfied | Clean allocation-tracked PF6 artifacts from `93b80dafcf50282088d96c68604f84bf0eed94e1` |
| Honest 1.25x gate disposition | Satisfied | Direct-match ratios 4.726x and 4.214x remain report-only with updated profiler notes |
| Final milestone audit and full-code review | Pending | Re-run both after evidence and documentation synchronization |
| Deferred provenance remains excluded | Satisfied | Checklist, ledger, and this report name full ErrorMatcher provenance as deferred |

## Remaining Work Outside PFM-B5

The selected PFM-B5 semantic scope has no intentionally unimplemented child case, but milestone-audit closure and final review sign-off are still pending.
Full ErrorMatcher stack-frame, heralded, and repeat-contained provenance plus `stim explain_errors` remain intentionally deferred.
The graphlike direct-match slowdown is an optimization backlog item and an explicit reason the rows remain outside the primary gate; it does not invalidate semantic closure.
PFM-B2 still owns the generated exhaustive gate-by-surface semantic matrix, and PFM-B6 still owns final audit and status rollup.
