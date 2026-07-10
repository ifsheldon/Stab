# PFM-B3 Shared Folded DEM Traversal Progress Report

## Summary

PFM-B3 replaces consumer-specific compact DEM recursion with one checked folded traversal tree and visitor policy.
The implementation owns exactly the seven `pfm4-traversal-*` cases in `docs/plans/blocker-closure-ledger.json`: counts, coordinates, compact transforms, sampler compilation, graphlike and hypergraph search collection, SAT and WCNF collection, and ErrorMatcher filter collection.
All seven cases are implemented with independently selectable Rust tests and focused oracle rows.

The shared traversal is internal to `stab-core`.
It does not add a new public DEM cursor API.

## Architecture

`crates/stab-core/src/dem/traversal.rs` builds a tree proportional to the compact DEM body.
Each block caches checked scalar summaries for detector shift, detector count, observable count, error count, thresholded detector declaration count, declaration bounds, maximum repeat depth, nonzero-error presence, and consumer fold eligibility.
Summary failures remain metric-local, so an overflow in one query does not prevent an unrelated compact transform from preserving the model.
Expanded declaration-count overflow is represented as "above the bounded local-scan limit" instead of a model error, allowing algebraic selected-coordinate lookup to continue.

The default visitor state carries only checked detector offset, folded-repeat depth, and folded multiplicity.
Coordinate vectors are opt-in state for coordinate APIs and are computed on demand under an 8,000,000 scalar-update budget with fallible vector growth; count, sampler, search, SAT/WCNF, and matcher consumers neither allocate nor update coordinate vectors.
Repeat depth is a scalar block summary validated only by consumers that historically own the 256-level cap, while count and compact transform APIs preserve their prior behavior for programmatic depth-257 models.
Visitors choose one repeat action:

- `Skip` for irrelevant bodies.
- `StructuralOnce` when the first structural iteration is sufficient.
- `FoldOnce` when the consumer can combine identical zero-detector-shift repetitions.
- Bounded `Expand` when the requested output or algorithmic state is inherently expanded.
- `Selected` for algebraically chosen repeat iterations in selected-coordinate queries.

Traversal preserves visitor errors and `ControlFlow` early termination immediately.
Detector shifts, coordinate shifts, repeat products, detector target rebasing, output counts, and folded multiplicities use checked arithmetic.

## Consumer Migration

| Consumer | Shared behavior | Consumer-specific behavior retained |
| --- | --- | --- |
| Count and final-shift APIs | Read cached folded summaries from one compact tree. | Each public call still returns its existing typed result and error class. |
| Selected coordinates | Uses declaration bounds, algebraic flat and bounded-nested scans, and selected repeat iterations from the shared tree. | First-declaration semantics and the one-million ambiguous-candidate guard remain source-owned. |
| Full coordinates | Uses the shared coordinate visitor after validating the materialized output count. | The one-million detector cap remains because Stim returns one map entry for every detector index, including empty coordinates. |
| `rounded` and `without_tags` | Use direct compact recursive transforms and preserve repeat structure. | They deliberately avoid building an auxiliary traversal tree, so peak allocation is the transformed body plus recursion state and prior depth-257 programmatic behavior remains intact. |
| `DetectorErrorModel::flattened` | No change to public semantics. | It remains capped because its public result is the expanded instruction stream. |
| `CompiledDemSampler` | Compiles from the shared tree and cached detector and observable summaries. | Probability and parity folding remains sampler-specific; sampled-error output and replay preserve flat occurrence order and their materialized width caps. |
| Graphlike and hypergraph collection | Shares inactive-body skipping, zero-shift folding, checked offsets, bounded expansion, and early errors. | Graph construction and exponential search-state limits remain separate from input traversal. |
| SAT and WCNF collection | Shares checked traversal, selected folding, and folded multiplicity. | Weighted MAP probability folding and clause generation remain SAT-specific; clause and dense-target caps protect expanded output. |
| ErrorMatcher filter collection | Shares fold eligibility, checked offsets, bounded expansion, and immediate errors. | Canonical parity keys remain matcher-specific; full ErrorMatcher provenance and repeat-contained circuit noise remain deferred. |
| Circuit analyzer and ErrorMatcher circuit provenance | Do not consume compact DEM input and therefore do not use the shared DEM tree. | Existing circuit traversal caps and deferred full provenance are separate from PFM-B3 input traversal. |

## Coordinate Contract Clarification

Pinned Stim v1.16.0 `DetectorErrorModel::get_detector_coordinates` returns an entry for every requested detector, and the no-filter Python form requests every detector index below `count_detectors()`.
Undeclared detector coordinates are returned as empty vectors.
Therefore, a sparse declaration at `D2000000` implies 2,000,001 entries in the full map and must remain subject to the materialized output cap.

The PFM-B3 requirement to avoid scanning nonexistent sparse detector ids applies to selected lookup.
Selected lookup now traverses the compact tree, prunes by declaration bounds, and visits only algebraically selected candidate iterations without flattening preceding repeats.

## Tests

The focused integration test file is `crates/stab-core/tests/dem_folded_traversal.rs`.
Its independently selectable tests are:

- `pfm_b3_folded_traversal_counts`
- `pfm_b3_folded_traversal_coordinates`
- `pfm_b3_folded_traversal_transforms`
- `pfm_b3_folded_traversal_sampler`
- `pfm_b3_folded_traversal_search`
- `pfm_b3_folded_traversal_sat`
- `pfm_b3_folded_traversal_matcher_filter`

The tests cover flat and nested bodies, empty and single-iteration repeats, zero and nonzero detector shifts, coordinate shifts, sparse detector ids, annotation-only and logical-only bodies, separator-bearing errors, mixed active and zero-probability errors, first coordinate declaration, deterministic generated differential models, arithmetic overflow, materialized output caps, shifted active-repeat rejection, visitor errors, and statistical sampler parity.
The count selector runs 96 deterministic Proptest cases with ChaCha seed `[0xB3; 32]`, generated depth 3, recursive size 48, branch width 4, root width 6, repeat counts 1 through 3, detector shifts 0 through 2, coordinate widths 0 through 3, tags, annotations, zero or deterministic active errors, separators, and detector or observable targets.
Each generated compact model is compared with an explicitly unrolled reference for scalar summaries, full coordinates, rounded and tag-stripped flattened output, deterministic sampler records, graphlike and hypergraph results, and ErrorMatcher filter results.
Additional regressions prove declaration-count overflow cannot block D0 or sparse-hole D1 lookup, ignored coordinate state cannot affect count or search, aggregate coordinate work is capped, empty and annotation-only repeats skip before consumer caps, depth-257 count and compact transforms retain prior behavior, and deep sampler failure retains `InvalidSamplerCompilation`.

The sampler test uses 100,000 shots and seed `12648437` for repeated joint detector-observable buckets at probabilities 0.5 and 0.5 plus the pinned `resample_combinations` detector marginals 0.34, 0.26, and 0.38 with an exact even-parity invariant.
Every non-deterministic bucket uses tolerance `max(0.01, 6 sigma)`; the exact two-sided familywise failure probability is below the declared `0.000001` budget.
The SAT selector asserts literal pinned Stim unweighted and quantization-100 weighted WCNF text before comparing folded and compact models.
Fractional coordinate parity uses absolute tolerance `1e-12` against pinned Stim's sequential accumulation because Stab's algebraic multiplication intentionally preserves bounded traversal at slightly different floating-point rounding.

Existing focused coordinate, sampler, search, SAT, matcher, parser, and hostile-input suites remain part of the regression evidence.

## Oracle Evidence

The PF4 oracle manifest now contains one focused row per case:

- `pfm-b3-dem-traversal-counts`
- `pfm-b3-dem-traversal-coordinates`
- `pfm-b3-dem-traversal-transforms`
- `pfm-b3-dem-traversal-sampler`
- `pfm-b3-dem-traversal-search`
- `pfm-b3-dem-traversal-sat`
- `pfm-b3-dem-traversal-matcher-filter`

`pf4-dem-folded-traversal` is an implemented umbrella selector only and does not substitute for the seven child rows.
The blocker ledger freezes each child row's runner, argv, parity mode, comparator, and pinned upstream source.

## Benchmark Evidence

`pfm-b3-dem-traversal-core` is a non-primary, contract-only row because pinned Stim does not expose a faithful Rust internal traversal baseline.
It has four submeasurements:

- `flat-equivalent`: 4,096 compact instructions, reported as compact instructions per second.
- `nested-large-repeat`: three compact body instructions representing 3,000,000,000,000,000,000 expanded instructions, reported as represented instructions per second.
- `sparse-selected-coordinate`: two compact declarations across 4,000,000 represented iterations for one selected detector.
- `wide-coordinate-irrelevant`: 128 nested repeat blocks around one 4,096-dimensional coordinate shift, measured through coordinate-free detector counting.

The preliminary allocation-enabled run on 2026-07-10 recorded:

| Submeasurement | Median | Peak live allocated bytes | Resident delta |
| --- | ---: | ---: | ---: |
| Flat equivalent | 41.870 microseconds | 65,536 | 0 |
| Nested large repeat | 0.165 microseconds | 784 | 0 |
| Sparse selected coordinate | 0.464 microseconds | 1,968 | 0 |
| Wide coordinate irrelevant | 9.609 microseconds | 47,120 | 0 |

The preliminary report is under `target/benchmarks/pfm-b3-dem-traversal-compare` and records `local_modifications=true`.
A clean committed-HEAD rerun is required before this report becomes final milestone evidence.
Existing consumer rows remain report-only and now describe the shared traversal path instead of claiming that true folded input traversal is pending.

## Verification

Completed during implementation:

```text
cargo test -p stab-core --test dem_folded_traversal --quiet
cargo test -p stab-core --quiet
cargo clippy -p stab-core --all-targets -- -D warnings
cargo test -p stab-oracle fixtures --quiet
just oracle::list --milestone PF4
just oracle::blockers
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo clippy -p stab-bench --all-targets -- -D warnings
just bench::smoke
just bench::compare-allocations --only pfm-b3-dem-traversal-core --warmup --measurement-runs 3 --report target/benchmarks/pfm-b3-dem-traversal-compare
```

## Audit And Review

Milestone-audit status: implementation and evidence findings resolved; final completion remains pending only on the clean committed-HEAD allocation rerun.

The audit and three GPT-5.6/max full-code-review sidecars found:

- Exact expanded detector-declaration counts could overflow before selected lookup reached its algebraic fallback.
- Eager coordinate summaries and unconditional coordinate visitor state could amplify a compact wide-coordinate model into repeat-count-scaled CPU work or depth-scaled retained memory.
- Empty and annotation-only neutral repeats reached search, SAT/WCNF, or ErrorMatcher caps instead of skipping.
- The shared depth guard changed prior depth-257 count and transform behavior and changed sampler failure from `InvalidSamplerCompilation` to `InvalidDetectorErrorModel`.
- Compact transforms built an unnecessary auxiliary tree, dead visitor state scanned targets, the generated differential corpus was under-defined, SAT evidence compared only two Stab paths, sampler evidence did not port its named upstream case, coordinate metadata overclaimed overflow, and historical reports still described the umbrella as manifest-only.
- Pinned Stim accumulates fractional coordinate shifts sequentially, while bounded algebraic folding multiplies them and can differ by floating-point rounding.

All implementation findings are fixed:

- Detector declaration counts are thresholded, coordinate vectors are opt-in and scalar-work-capped with fallible growth, and the wide-coordinate benchmark proves coordinate-free traversal allocation.
- Neutral repeats skip before consumer caps, depth ownership is consumer-specific, sampler errors retain their public variant, compact transforms recurse directly, and dead state is removed.
- The 96-case generated corpus, literal pinned WCNF assertions, actual `DemSampler.resample_combinations` port, coordinate overflow and tolerance checks, and synchronized current and historical documentation close the evidence findings.
- The full-map interpretation, generated-corpus domain, numeric comparator, Rust-test-proxy comparator limitation, and statistical-schema limitation are logged in `docs/plans/milestone-spec-gaps.md`.

The three initial GPT-5.6/max reviews completed and supplied the findings above.
A requested focused sidecar closure rerun could not start because the subagent account reached its usage limit; local closure verification reran the focused regressions, full core suite, Clippy, blocker validator, oracle metadata tests, benchmark harness tests, and dirty allocation probe instead.
This unavailable optional second pass does not replace or weaken the completed initial full review, but it is recorded here rather than hidden.

| Requirement | Status before clean rerun | Evidence |
| --- | --- | --- |
| Shared bounded-result traversal | Satisfied | `crates/stab-core/src/dem/traversal.rs`; seven focused selectors |
| Sparse coordinate and overflow policy | Satisfied | `pfm_b3_folded_traversal_coordinates`; thresholded declaration count; 8,000,000 scalar-update cap |
| Compact transform allocation | Satisfied | direct `rounded` and `without_tags`; depth-257 regression |
| Sampler, search, SAT/WCNF, and matcher semantics | Satisfied | statistical combinations, neutral-repeat regressions, literal WCNF, shifted-active cap tests |
| Generated differential corpus | Satisfied | 96 deterministic Proptest cases under `pfm_b3_folded_traversal_counts` |
| Oracle and ledger evidence | Satisfied | seven independent rows; `just oracle::blockers --check-selectors` |
| Benchmark contract and memory evidence | Preliminary | four-submeasurement dirty allocation report; clean committed-HEAD rerun pending |
| Documentation synchronization | Satisfied | plan, checklist, inventory, roadmap, test map, rollup, current report, and historical closure notes |
