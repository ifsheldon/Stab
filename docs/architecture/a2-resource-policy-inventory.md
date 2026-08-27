# A2 Resource Policy Inventory

This inventory records the A2 review of production safety constants in `stab-core` and the implemented CLI.

Its purpose is to distinguish caller-selectable experiment budgets from semantic, representation, platform, and implementation safety contracts.

The presence of a numeric constant does not by itself justify a public policy field.

## Classification Rules

- **Caller-selectable policy:** lowering the value is useful for admission control, raising it can remain valid within the operation's semantic and platform invariants, and the operation can reject before materialization or mutation.
- **Semantic or representation invariant:** changing the value changes the Stim dialect, an identifier representation, a format contract, or mathematical validity.
- **Recursive safety envelope:** downstream code still relies on a fixed stack or nesting bound, so callers may tighten but not relax it.
- **Fixed operation safety contract:** the current implementation needs a bounded fallback or materialization ceiling, but callers cannot yet safely relax it or there is no demonstrated need for a public choice.
- **I/O boundary:** a path-based convenience or CLI route has a fixed hostile-input cap. This is user-visible admission, but it is not a reusable algorithm policy.
- **Implementation threshold:** the value selects storage, preallocation, or a backend fast path and is not an accepted-work limit.

## Caller-Selectable Policies

| Policy and owner | Configurable dimensions and defaults | Why it is public |
| --- | --- | --- |
| `ParseLimits`, model parsing | 64 MiB original source; 1,000,000 physical lines; 1,000,000 compact declarations; 32,000,000 retained targets; repeat nesting up to the shared 256-level hard maximum | Callers can cap hostile source and compact model growth before proportional work and retention. Byte admission precedes byte-metadata preparation; that preparation is bounded by source-byte and admitted-line policy and intentionally precedes semantic declaration and target admission. Top-level model storage grows only from successfully parsed declarations, blank and comment-only source reserves no model storage, and each decoded target is admitted before retention. Byte, line, declaration, and target limits may be tightened or relaxed; repeat nesting may only be tightened while recursive consumers retain the shared envelope. |
| `CircuitFlattenLimits`, circuit flatten analysis | 1,000,000 materialized operations; 32,000,000 retained target occurrences; 16,000,000 retained argument values; 512 MiB conservative materialized bytes | Flattening is explicitly materializing, and callers can choose smaller or larger output and payload budgets before the output circuit is allocated. Both materialized circuit and operation-vector adapters first admit programmatic models against the fixed 256-level recursive envelope and the platform's maximum instruction-vector capacity. The adapters use fallible reservation after admission. |
| `DemFlattenLimits`, DEM flatten analysis | repeat unroll 100,000; expanded instructions 1,000,000; aggregate repeat iterations 1,000,000; 32,000,000 retained target occurrences; 16,000,000 retained argument values; 512 MiB conservative materialized bytes | The operation explicitly expands folded input, and traversal and retained-payload dimensions can be admitted before appending output. |
| `DetectionConversionLimits`, detection compilation and conversion | fixed 256-level repeat-nesting safety envelope; record width 1,000,000 bits; expanded instructions 1,000,000; aggregate repeat iterations 1,000,000; compact retained measurement terms 16,000,000; conservative compact-plan bytes 256 MiB | Callers may independently bound per-record shape, traversal work, and retained compact-plan storage. A folded body is retained once, so repeat count is not an independent syntax cap; counts above 100,000 are accepted when aggregate admitted work and record shape fit. Repeat nesting is a non-configurable model-safety invariant and is admitted iteratively before conversion or detector-frame materialization. Direct detector-frame compilation charges the compact conversion tape plus one compact executable frame tape, including each retained instruction's arguments and targets, before fallible construction; repeat counts are executed only by the bounded runtime cursor. Sweep-conditioned conversion admits the same frame tape for per-record detector and observable corrections. Canonical sessions retain bounded batch scratch while output retention belongs to the caller, so the retired whole-output materializer and `max_materialized_bits` policy no longer exist. Finite aggregate traversal defaults prevent compact hostile repeats from driving unbounded CPU without forcing materialization. |
| circuit sampling | expanded operations per measurement-bearing shot 1,000,000; zero-width execution work 0 | The fixed aggregate boundary prevents one compact circuit from monopolizing an uncancellable batch without restoring a repeat-count syntax cap. Zero-width plans validate their compact operation tape but allocate no frame and do not traverse represented operations because no result can observe them. Counts above `u64::MAX` retain an explicit lower-bound marker in typed and JSON diagnostics instead of reporting the cap as an exact value. |
| `DemSamplerLimits`, compiled DEM sampling | sampled error applications 64,000,000; replay work units 64,000,000; active batch bytes 64 MiB | Sampling work, replay traversal, and reusable session storage are distinct budgets. Caller-owned sinks decide whether output is streamed or retained, so the engine has no total-output budget. Replay input remains caller-owned while operation-owned traversal admission bounds its semantic work. |
| `LogicalErrorSearchLimits`, graphlike and hypergraph analysis | repeat unroll 100,000; repeat iterations 1,000,000; nonzero error mechanisms 5,000,000; target occurrences per mechanism 65,536; total target occurrences 20,000,000; effective detector nodes 1,000,000; unique graph edges 5,000,000; stored graph terms 20,000,000; hyperedge degree 4,096; hyperedge incidences 5,000,000; search states 1,000,000; transitions 20,000,000; terms per state 65,536; stored state terms 5,000,000 | Search has several independently growing retained structures. Named fields let research callers trade completeness against bounded memory and work without changing graph semantics. |
| `SatMaterializationLimits`, SAT analysis | repeat unroll 100,000; expanded instructions 1,000,000; repeat iterations 1,000,000; error mechanisms 250,000; target occurrences 500,000; variables 500,000; clauses 500,000; clause literals 1,500,000; WDIMACS output 128 MiB | SAT construction is intentionally materializing. Its traversal and CNF shape are admitted before retained construction, and serialized output is admitted from the exact encoded byte count before allocation. |

All default entry points delegate through these exact defaults.

Every policy rejection exposes `ResourceLimitError` with a stable operation, resource kind, actual amount, and limit. Model-parser failures additionally retain the dialect, optional source line, and rejected byte span.

## Boundary Evidence Rules

Default accepted maxima and first rejections are executed directly whenever the fixture and retained output fit safely in the ordinary test suite.

A resource-prohibitive or representational maximum may use a reduced policy boundary only when the owning test also proves the exact default value, accepts `N`, rejects `N + 1`, exercises checked arithmetic and overflow for the same admission path, and documents why constructing the default boundary would itself violate the test suite's memory or runtime budget.

This exception applies to dimensions such as representational `u64::MAX` traversal defaults and large exact serialized-output ceilings. It does not permit replacing a practical finite historical boundary with a smaller convenient test.

The acceptance test for each policy must charge only resources owned by that operation. Replay input, caller-owned buffers, streaming output, and folded source structure are not reclassified as materialized output merely to make a boundary easier to test.

For this inventory, an ordinary boundary test does not intentionally retain more than 64 MiB solely to prove a production ceiling. Larger real workloads belong in bounded memory diagnostics; the ordinary suite uses exact reduced boundaries plus arithmetic and platform-capacity evidence.

## Per-Dimension Evidence

The selector column names the narrowest current test that exercises the dimension.

`Yes` in the real-default column means the test executes an accepted request exactly at the production default maximum, not merely that it asserts the constant or rejects the first excess.

`No` means the production maximum is not intentionally materialized by the ordinary test suite.

Such a row is closed only when the named selector proves the exact configured value, an accepted reduced `N`, the first rejected `N + 1`, checked arithmetic or an earlier dominating platform-capacity guard, and a concrete retained-memory, runtime, or representational reason for the substitution.

Any row that does not meet that contract is labeled `Open`.

### ParseLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Original source bytes | 64 MiB | `cargo test -p stab-model --test parse_resources parser_admission_accepts_exact_limits_and_rejects_the_first_excess_unit -- --exact` | No | The capabilities contract publishes the production default. This owner accepts exact reduced byte limits and rejects the next byte before UTF-8 decoding through both dialects; the allocation owner separately proves that a rejected byte suffix allocates nothing. Materializing a separate 64 MiB fixture would add cost without exercising a different branch. |
| Physical source lines | 1,000,000 | `cargo test -p stab-model --test model_parse_diagnostics byte_parse_limit_entrypoints_apply_custom_limits_after_utf8_decode -- --exact` | No | Both dialects accept a configured two-line boundary and reject line three with exact typed context. A million-line fixture would test repetition rather than another admission branch. |
| Compact declarations | 1,000,000 | `cargo test -p stab-model --test parse_resources parser_admission_accepts_exact_limits_and_rejects_the_first_excess_unit -- --exact` | No | Both dialects count repeat headers and body declarations before expansion. `parser_admission_accumulates_fast_path_targets_and_prefusion_declarations` separately proves adjacent circuit declarations remain distinct before fusion, and the allocation owner proves a large post-rejection suffix is not retained. |
| Retained targets | 32,000,000 | `cargo test -p stab-model --test parse_resources parser_admission_accepts_exact_limits_and_rejects_the_first_excess_unit -- --exact` | No | Circuit combiners and DEM separators count as represented targets. Both dialects admit three and reject the fourth at its exact token before retention; `parser_admission_accumulates_fast_path_targets_and_prefusion_declarations` proves cumulative charging through canonical fast paths. A 32-million-target fixture would require substantial model storage without exercising another branch. |
| Repeat nesting | 256 | `cargo test -p stab-model --test model_parse_diagnostics default_repeat_depth_rejects_the_first_unsafe_header_without_stack_overflow -- --exact` | Yes | Circuit and DEM parsing reject level 257 with exact typed context and source spans; owner-level folded traversal separately proves the accepted boundary. |

### CircuitFlattenLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Expanded operations | 1,000,000 | `cargo test -p stab-analysis --test circuit_flatten_limits policy_preserves_defaults_and_rejects_before_output_allocation -- --exact` | No | Closed by substitution. The selector asserts the exact default and default first rejection, accepts custom 3, rejects 4, and proves rejected target payload does not scale allocation. `operation_count_overflow_fails_before_materialization` owns checked operation arithmetic. Materializing one million owned `CircuitItem` values plus a separately reserved instruction vector intentionally exceeds the ordinary single-test retained-memory budget. |
| Retained target occurrences | 32,000,000 | `cargo test -p stab-analysis --test circuit_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector accepts six target occurrences and rejects the first excess at five before reservation. Thirty-two million retained `Target` values require `32_000_000 * size_of::<Target>()` bytes before circuit items and allocator overhead. `retained_payload_count_overflow_fails_before_materialization` owns checked target multiplication. |
| Retained argument values | 16,000,000 | `cargo test -p stab-analysis --test circuit_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector accepts three argument values and rejects the first excess at two before reservation. Sixteen million `f64` values require at least 128 MiB before circuit items and allocator overhead. `retained_payload_count_overflow_fails_before_materialization` owns checked argument multiplication. |
| Conservative materialized bytes | 512 MiB | `cargo test -p stab-analysis --test circuit_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector computes one operation's exact conservative footprint, accepts that byte count, and rejects one byte below it. The 512 MiB production ceiling is itself above the ordinary single-test retained-memory budget; for caller-raised arithmetic, platform instruction-vector capacity rejects before an overflowing byte product can become materializable. |

### DemFlattenLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Repeat unroll | 100,000 | `cargo test -p stab-analysis --test dem_flatten_limits practical_default_repeat_boundaries_are_executed_exactly -- --exact` | Yes | An empty-body model executes the real repeat maximum without manufacturing retained output; `default_entry_points_are_equivalent_and_keep_default_error_text` owns the first default rejection. |
| Expanded instructions | 1,000,000 | `cargo test -p stab-analysis --test dem_flatten_limits each_limit_accepts_its_exact_boundary_and_rejects_the_first_excess -- --exact` | No | Closed by substitution. The selector accepts custom six and rejects five, while `repeat_multiplier_overflow_is_rejected_before_materialization` owns checked repeat multiplication and `caller_raised_limit_cannot_exceed_platform_materialization_capacity` owns the dominating vector-capacity guard. One million owned `DemItem` values exceed the ordinary 64 MiB single-test budget before their target, argument, and tag payloads. |
| Aggregate repeat iterations | 1,000,000 | `cargo test -p stab-analysis --test dem_flatten_limits practical_default_repeat_boundaries_are_executed_exactly -- --exact` | Yes | A nested empty-body model executes exactly one million aggregate iterations and rejects 1,000,001 without retaining expanded instructions. |
| Retained target occurrences | 32,000,000 | `cargo test -p stab-analysis --test dem_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector accepts six retained targets and rejects the first excess at five. Thirty-two million `DemTarget` values exceed the ordinary test budget before item and allocator overhead; the platform item-vector guard dominates caller-raised products that cannot be represented. |
| Retained argument values | 16,000,000 | `cargo test -p stab-analysis --test dem_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector accepts six argument values and rejects the first excess at five. Sixteen million `f64` values require at least 128 MiB before item and allocator overhead; the platform item-vector guard dominates caller-raised products that cannot be represented. |
| Conservative materialized bytes | 512 MiB | `cargo test -p stab-analysis --test dem_flatten_limits retained_payload_dimensions_have_exact_boundaries -- --exact` | No | Closed by substitution. The selector accepts the exact conservative bytes for six instructions and rejects one byte below that requirement. Intentionally allocating 512 MiB exceeds the ordinary single-test budget, and the caller-raised platform-capacity selector fails before an unrepresentable result can reach reservation. |

### DetectionConversionLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Record width | 1,000,000 bits | `cargo test -p stab-engine --test detection_resource_limits default_record_width_accepts_the_boundary_and_rejects_the_next_bit -- --exact` | Yes | A single wide measurement executes the real default and rejects 1,000,001 before compiled-plan construction; `a2_detection_compile_limits_are_independent_and_atomic` isolates every smaller custom boundary and its typed policy context in one canonical owner. |
| Expanded instructions | 1,000,000 | `cargo test -p stab-engine --test detection_resource_limits default_traversal_limits_accept_the_boundary_and_reject_the_next_unit -- --exact` | Yes | A no-term `TICK` fixture executes exactly one million instructions through admission and construction and rejects 1,000,001 with typed context; `a2_detection_compile_limits_are_independent_and_atomic` isolates the configured dimension. |
| Aggregate repeat iterations | 1,000,000 | `cargo test -p stab-engine --test detection_resource_limits default_traversal_limits_accept_the_boundary_and_reject_the_next_unit -- --exact` | Yes | A compact nested empty-body fixture executes exactly one million aggregate repeat iterations and rejects 1,000,001 without retaining terms; `a2_detection_compile_limits_are_independent_and_atomic` isolates the configured dimension. |
| Compact retained terms | 16,000,000 | `cargo test -p stab-engine --test canonical_owner_evidence a2_detection_compile_limits_are_independent_and_atomic -- --exact` | No | Closed by substitution. The folded selector retains its two measurement terms once, accepts exactly two, and rejects the first excess at one in the allocation-free dry admission pass. Sixteen million record offsets alone require at least 128 MiB on supported 64-bit hosts before operations, repeat markers, and allocator overhead. |
| Conservative compact-plan bytes | 256 MiB | `cargo test -p stab-engine --test canonical_owner_evidence a2_detection_compile_limits_are_independent_and_atomic -- --exact` | No | Closed by substitution. The selector discovers and accepts the exact flat operation-tape and term footprint, then rejects one byte below that requirement before retained construction. Intentionally allocating 256 MiB exceeds the ordinary single-test budget. |

### DemSamplerLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Sampled error applications | 64,000,000 | `cargo test -p stab-engine --test canonical_owner_evidence a2_dem_sampler_work_policy_admission -- --exact` | No | The custom three/four fixture proves exact rejection before RNG advancement or sink mutation. Executing 64 million stochastic applications would make the ordinary suite CPU-bound. |
| Replay work units | 64,000,000 | `cargo test -p stab-engine --test canonical_owner_evidence a2_dem_sampler_replay_work_policy_admission -- --exact` | No | The custom four/three fixture proves exact traversal admission before sink mutation, while `cargo test -p stab-engine --lib dem_sampling::tests::replay_work_arithmetic_overflow_rejects_before_execution -- --exact` owns checked multiplication. Constructing and traversing 64 million caller-owned replay units would consume substantial memory and runtime. |
| Active batch bytes | 64 MiB | `cargo test -p stab-engine --test dem_sampling_plan_session session_batch_capacity_obeys_the_caller_active_byte_budget -- --exact` | No | A focused exact fixture proves that the budget controls reusable batch capacity and rejects the first one-shot excess. Allocating the full 64 MiB ceiling solely to prove admission would consume the ordinary single-test budget. |

### LogicalErrorSearchLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Repeat unroll | 100,000 | `cargo test -p stab-analysis dem::search::error_traversal::tests::search_repeat_limits_execute_practical_default_maxima -- --exact` | Yes | A compact shifting-error model visits exactly 100,000 repeated mechanisms and rejects repeat count 100,001 before invoking the visitor. |
| Aggregate repeat iterations | 1,000,000 | `cargo test -p stab-analysis dem::search::error_traversal::tests::search_repeat_limits_execute_practical_default_maxima -- --exact` | Yes | A nested compact model executes exactly one million aggregate repeat iterations and rejects the first excess; the smaller custom selector independently isolates 8/7 accounting. |
| Expanded nonzero error mechanisms | 5,000,000 | `cargo test -p stab-analysis dem::search::error_traversal::tests::search_traversal_has_a_distinct_error_mechanism_cap -- --exact` | No | Closed by substitution. The selector accepts 10,000 forwarded nonzero mechanisms and rejects 10,001. Visiting five million callbacks would make the ordinary suite CPU-bound, while no mechanism storage is retained by this traversal owner. |
| Target occurrences per mechanism | 65,536 | `cargo test -p stab-analysis dem::search::error_traversal::tests::search_traversal_rejects_large_error_target_lists_before_normalization -- --exact` | No | Closed by substitution. The selector accepts 128 targets and rejects 129 before normalization. Constructing a 65,536-target parsed instruction is unnecessary because the exact per-instruction length is already known and checked before forwarding. |
| Total target occurrences | 20,000,000 | `cargo test -p stab-analysis dem::search::error_traversal::tests::search_traversal_bounds_aggregate_error_target_work -- --exact` | No | Closed by substitution. The selector accepts 10,000 total target occurrences and rejects 10,002. Visiting twenty million target occurrences would make the ordinary suite CPU-bound, while the traversal does not retain them. |
| Effective detector nodes | 1,000,000 | `cargo test -p stab-analysis dem::search::hyper::tests::effective_detector_limit_is_exact_and_independent -- --exact` | No | The custom 2/1 fixture proves an exact independent boundary without allocating a million-node graph, and the public-entry-point propagation test proves the policy reaches both search implementations. |
| Unique graph edges | 5,000,000 | `cargo test -p stab-analysis dem::search::budget::tests::graph_construction_budget_enforces_edge_and_payload_limits -- --exact` | No | The custom 64/65 fixture proves exact budget commit without retaining five million edges, which would dominate test memory. |
| Stored graph terms | 20,000,000 | `cargo test -p stab-analysis dem::search::budget::tests::graph_construction_budget_enforces_edge_and_payload_limits -- --exact` | No | The custom 2,048/2,049 fixture proves aggregate payload accounting without retaining twenty million terms, which would dominate test memory. |
| Hyperedge degree | 4,096 | `cargo test -p stab-analysis dem::search::hyper::limit_policy_tests::default_hyperedge_degree_boundary_is_executed_exactly -- --exact` | Yes | One 4,096-detector edge is practical in the ordinary suite, so the selector executes the production maximum and rejects 4,097 before mutating the graph. The separate custom-policy selector retains focused degree and incidence independence coverage. |
| Hyperedge incidences | 5,000,000 | `cargo test -p stab-analysis dem::search::hyper::tests::hyperedge_degree_and_incidence_rejections_leave_graph_unchanged -- --exact` | No | The reduced exact incidence fixture avoids a five-million-entry adjacency structure, which would dominate test memory. |
| Search states | 1,000,000 | `cargo test -p stab-analysis dem::search::budget::tests::search_budget_enforces_state_and_transition_limits -- --exact` | No | The custom 64/65 fixture proves exact admission without retaining one million state records, which would dominate test memory. |
| Search transitions | 20,000,000 | `cargo test -p stab-analysis dem::search::budget::tests::search_budget_enforces_state_and_transition_limits -- --exact` | No | The custom 4,096/4,097 fixture proves exact accounting without executing twenty million transitions, which would make the ordinary suite CPU-bound. |
| Terms per search state | 65,536 | `cargo test -p stab-analysis dem::search::budget::tests::search_budget_enforces_per_state_and_aggregate_payload_limits -- --exact` | No | The custom 64/65 fixture proves exact per-state accounting without constructing a 65,536-term state. |
| Stored search-state terms | 5,000,000 | `cargo test -p stab-analysis dem::search::budget::tests::search_budget_enforces_per_state_and_aggregate_payload_limits -- --exact` | No | The custom 256/257 fixture proves exact aggregate accounting without retaining five million terms, which would dominate test memory. |

The logical-search overflow selectors are `cargo test -p stab-analysis dem::search::error_traversal::tests::search_traversal_counter_overflow_rejects_before_forwarding -- --exact` and `cargo test -p stab-analysis dem::search::budget::tests::budget_overflow_rejects_without_committing_partial_state -- --exact`.

Together they cover mechanism, target, graph-edge, stored-graph-term, search-state, and transition arithmetic without forwarding rejected traversal work or committing partial retained state.

### SatMaterializationLimits

| Dimension | Default | Exact test selector | Real default max executed? | Reduced-boundary justification or remaining gap |
| --- | ---: | --- | --- | --- |
| Repeat unroll | 100,000 | `cargo test -p stab-analysis --test sat_materialization_limits traversal_limits_accept_exact_maxima_and_reject_first_excesses -- --exact` | No | The custom 3/2 fixture isolates repeat admission and the default entry-point test proves the first default rejection; materializing 100,000 SAT error instances is excluded from the ordinary suite. |
| Expanded instructions | 1,000,000 | `cargo test -p stab-analysis --test sat_materialization_limits traversal_limits_accept_exact_maxima_and_reject_first_excesses -- --exact` | No | The custom 2/1 fixture proves exact admission without constructing a million-instruction SAT input, which would dominate runtime and retained CNF state. |
| Aggregate repeat iterations | 1,000,000 | `cargo test -p stab-analysis --test sat_materialization_limits traversal_limits_accept_exact_maxima_and_reject_first_excesses -- --exact` | No | The custom 6/5 nested fixture proves aggregate accounting without executing one million traversal iterations. |
| Error mechanisms | 250,000 | `cargo test -p stab-analysis --test sat_materialization_limits flattened_error_and_target_limits_are_admitted_before_collection -- --exact` | No | The custom 2/1 fixture proves exact pre-collection admission without retaining 250,000 error structures. |
| Target occurrences | 500,000 | `cargo test -p stab-analysis --test sat_materialization_limits flattened_error_and_target_limits_are_admitted_before_collection -- --exact` | No | The custom 2/1 fixture proves exact target accounting without retaining 500,000 targets. |
| Variables | 500,000 | `cargo test -p stab-analysis --test sat_materialization_limits cnf_shape_limits_accept_exact_maxima_and_reject_first_excess -- --exact` | No | The custom 3/2 fixture proves exact shape admission without constructing a 500,000-variable CNF. |
| Clauses | 500,000 | `cargo test -p stab-analysis --test sat_materialization_limits cnf_shape_limits_accept_exact_maxima_and_reject_first_excess -- --exact` | No | The custom 8/7 fixture proves exact shape admission without constructing a 500,000-clause CNF. |
| Clause literals | 1,500,000 | `cargo test -p stab-analysis --test sat_materialization_limits cnf_shape_limits_accept_exact_maxima_and_reject_first_excess -- --exact` | No | The custom 16/15 fixture proves exact shape admission without retaining 1.5 million literals. |
| WDIMACS output | 128 MiB | `cargo test -p stab-analysis --test sat_materialization_limits every_early_unsat_path_obeys_the_output_byte_limit -- --exact` | No | Exact small accepted and first-rejected byte counts cover early and ordinary outputs, while allocating and serializing 128 MiB merely to prove the ceiling is excluded from the ordinary suite. |

The SAT checked-arithmetic selector is `cargo test -p stab-analysis --test sat_materialization_limits arithmetic_overflow_is_rejected_without_mutating_the_source -- --exact`.

The selectors above execute practical production maxima directly where the row says `Yes`. Large retained or repetitive maxima use the stated exact reduced-boundary substitution instead of manufacturing expensive tests.

Every remaining reduced boundary is closed by the substitution rule with an exact configured value, an accepted custom `N`, a rejected `N + 1`, checked arithmetic or a dominating platform-capacity guard, and a concrete resource reason recorded in its row.

## Semantic And Representation Invariants

| Source family | Fixed contract | Rationale |
| --- | --- | --- |
| `ids.rs`, `target.rs`, circuit parsing | Stim target values below $2^{24}$ | This is part of the frozen Stim target representation and accepted dialect. |
| `dem.rs`, `dem/parser.rs` | DEM detector IDs through $2^{62}-1$ and textual integers through $2^{60}-1$ | These bounds preserve the existing DEM representation, offset arithmetic, and pinned parser behavior. |
| probability and argument validation | finite values and operation-specific probability domains | These are semantic validity rules, not resource budgets. |
| result formats | exact record widths, namespace bounds, integer overflow rejection, newline grammar, and PTB64 groups of exactly 64 records | Relaxing these values would change public Stim-compatible file formats. |
| stabilizer algebra | valid phase, commutation, shape, and dimensional relationships | Mathematical validity cannot be overridden by a resource policy. |
| circuit generation | family parameter domains and valid code geometry | Invalid code parameters do not become valid when more resources are available. |

## Recursive Safety Envelopes

| Source family | Fixed contract | Rationale |
| --- | --- | --- |
| parsed circuit and DEM models | 256 repeat levels | Parsing may accept a tighter caller limit, but cannot raise the established parsed-model envelope while downstream recursive consumers still depend on it. Programmatically constructed models may be deeper only where an existing public API owns that behavior. Circuit flattening rejects before recursive work; folded DEM summary construction and destruction are iterative so compact count queries preserve their established depth-257 and deeper behavior. |
| feedback inlining, flow generation, and missing-detector analysis | 256 repeat levels | These consumers retain their own explicit fixed checks until their traversal implementations are made stack-independent. |
| DEM analyzer, sampler, ErrorMatcher, SAT, and expansion-owning traversal consumers | shared parsed-model repeat envelope where the consumer historically owns it | These consumers validate their fixed depth contract before recursive or expansion work. The folded summary representation itself has no 256-level construction cap because compact count queries historically accept deeper programmatic models. |

## Fixed Operation Safety Contracts

| Owner | Current fixed dimensions | Why A2 does not expose another policy |
| --- | --- | --- |
| feedback inlining | expanded work units 1,000,000; aggregate repeat iterations 1,000,000; repeat nesting 256 | There is no per-block repeat-count cap. The typed fixed-operation limits bound the current expansion algorithm; a public override would imply a completeness contract the operation does not yet provide. |
| detecting-region extraction | represented work 1,000,000; traversal work 1,000,000; live-state units 1,000,000; output records 1,000,000; output bytes 256 MiB; materialized Pauli width 1,048,576; repeat nesting 128; bounded all-target helpers | Compact repeat count is admitted independently. The limits protect recurrence probes, retained sparse state, and convenience output materialization; a later pass API can expose a policy if a real caller needs it. |
| missing-detector analysis | expanded declaration scan 1,000,000; projected flow storage 512 MiB; repeat nesting 256 | Covered final repeats use a count-independent fixed-point proof over one body iteration. Other shapes return a typed `missing-detector-discovery` resource error before expansion. The limits bound a specialized analysis fallback rather than a general reusable materialization seam. |
| circuit flow generation and checking | projected owned flow storage 512 MiB; expanded measurement-rich operations 1,000,000; repeat nesting 256; algebra-owned tableau, Pauli-bit, and row ceilings | These typed fixed-operation limits select bounded dense algorithms with sparse or fail-closed behavior. They are not interchangeable experiment budgets. |
| Pauli conjugation through nonunitary circuits | expanded instructions 1,000,000 | Pure Clifford repeats stay compact through one tableau action. The typed fixed-operation limit bounds only the measurement/reset traversal that currently requires expansion. |
| circuit time reversal | tableau qubits, expanded unitary work, and measurement-rich expansion | These limits define which current implementation path is supported. Raising them without a new algorithm would not be a safe caller choice. |
| circuit-to-DEM analysis | expanded instructions and aggregate repeat iterations 1,000,000; repeat nesting 256; bounded cycle probes | Foldable loops stay compact; fallback expansion is admitted by aggregate typed limits without a separate per-block repeat cap. These values remain fixed until a caller-selectable analysis plan owns them coherently. |
| ErrorMatcher and its filter | repeat unroll 100,000; expanded instructions and repeat iterations 1,000,000 | Full ErrorMatcher provenance remains intentionally partial. Exposing a policy now would prematurely stabilize that incomplete surface. |
| DEM coordinate queries | all-map detectors 1,000,000; selected-query candidates/declarations 1,000,000; coordinate scalar work 8,000,000 | These are convenience-query and folded-index safeguards. Selected APIs already provide the bounded alternative to all-map materialization. |
| stabilizer values and solvers | Pauli/Clifford 1,048,576 qubits; tableau/solver 512; random tableau 64; unitary matrix dimension 64; flow terms 65,536; repeat work 16,777,216 | These are representation, dense-algorithm, and numerical safety boundaries. They are exposed through typed algebra resource errors where applicable, but are not safely relaxable policy inputs. |
| generated circuits | 131,072 physical qubits | This prevents runaway materialization in fixed code-family generators. A future generator plan may own a policy if generated streaming or alternate storage makes a larger request meaningful. |

These fixed contracts are not forgotten work.

A future milestone may promote one only after identifying the owning operation, proving admission before expensive work, preserving exact defaults, and demonstrating a caller that benefits from the choice.

## I/O Boundaries

| Owner | Current cap | Classification |
| --- | --- | --- |
| `Circuit::from_file` and circuit CLI input | 64 MiB | Fixed hostile-input cap for path-based whole-file parsing. Streaming model parsing is not implemented. |
| `convert` CLI record input | 64 MiB per text record | Result-data conversion streams through the shared per-record reader (decision D5), so total input length is unbounded while memory stays bounded by one record frame; the `--in_format=stim` circuit route keeps the 64 MiB circuit-input cap. |
| `analyze_errors` CLI input | 64 MiB | Fixed route-level cap before model parsing and analysis. |
| `sample_dem` CLI model input | 64 MiB | Fixed route-level cap; sampled records themselves stream. |
| text replay and `m2d` records | 1 MiB per record | Fixed streaming-record cap that bounds one retained line rather than total input length. |

These caps belong to CLI and file conveniences, not to model, engine, or analysis policy objects.

## Implementation Thresholds

Word width, 64-qubit small-frame selection, 16-byte inline DEM tags, boxed rare opaque-tag payloads, geometric model-vector growth, float formatting buffers, transpose tile size, and probability-generator bucket counts are implementation choices.

They may affect performance and allocation but do not reject otherwise valid work solely because the threshold is crossed.

They therefore stay private and are covered by semantic equivalence, allocation, and benchmark tests instead of public resource-policy APIs.

## Review Outcome

A2 introduces seven concrete policies and no global `ResourcePolicy`, `CompileLimits`, generic `SamplingLimits`, generic `MaterializationLimits`, or generic `SearchLimits`.

This keeps caller control aligned with real operations and leaves fixed safety contracts honest about what the current implementation can support.
