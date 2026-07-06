# PFM4 DEM SAT High-Observable Cap Progress Report

## Scope

This PFM4 slice owns one resource-boundary evidence gap revealed after the selected unweighted zero-probability SAT repeat fold: folded flat zero-shift structural repeats that reference observable ids beyond the dense SAT observable-vector cap.

The selected behavior is rejection, not new folded output:

- `shortest_error_sat_problem` may fold a selected flat zero-shift repeat body containing `error(0)` structurally.
- If the folded structural body still implies more than `MAX_SAT_DENSE_TARGET_COUNT` effective observable nodes, SAT generation must reject before allocating dense observable vectors.
- The rejection must be source-owned by a focused unit test, because the previous slice had high-detector cap coverage but no dedicated high-observable cap test.

This slice does not change weighted SAT behavior, graphlike search, hypergraph search, analyzer traversal, ErrorMatcher traversal, DEM sampling, detectorless search folds, shifted repeat folds, nested repeat folds, non-flat repeat folds, Python APIs, diagrams, or deferred simulator-product surfaces.

## Comparator And Evidence Plan

Comparator class: structural Rust resource-boundary evidence.

The expected result is a precise domain error containing the dense SAT observable cap message.
No benchmark is required because this is a rejection-path resource guard, not a throughput path.

## Implemented Slice

Added `sat_problem_shortest_rejects_large_flat_zero_shift_zero_probability_high_observable_repeat`.
It constructs a large flat zero-shift repeat body containing `error(0) L1000001` and proves `shortest_error_sat_problem` rejects with the dense observable cap error.
This confirms selected structural folding reaches target validation and fails before repeat expansion or dense observable-vector allocation.

## Documentation And Metadata

Updated the existing SAT flat-repeat reports and checklist wording to name the dedicated high-observable cap evidence.
Reused the existing SAT-sourced oracle row `pf4-dem-sat-flat-repeat-fold-rust`; no new oracle row is needed because the existing row runs the `flat_zero_shift` test filter and already owns selected SAT flat-repeat folding evidence.
No benchmark row was added because this is a rejection-path resource guard, not a throughput path.

## Verification

Focused commands run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p stab-core sat_problem_shortest_rejects_large_flat_zero_shift_zero_probability_high_observable_repeat --quiet
cargo test -p stab-core sat_problem --quiet
cargo test --workspace --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
git diff --check
just maintenance::pre-commit
```

Milestone-audit status: complete for this evidence slice.
The audit found the slice satisfied its narrow PFM4 contract: source-owned coverage now proves selected high-observable zero-probability structural repeats reject at the dense observable cap, the docs do not claim broader folded traversal, and no benchmark row is required for this rejection path.

Full-code-review status: findings resolved.
The core GPT-5.5/xhigh sidecar found no core correctness, compatibility, resource-exhaustion, or test-quality issues and confirmed that the test proves the selected fold reaches the cap path while code inspection proves validation runs before dense vector allocation.
The docs and oracle GPT-5.5/xhigh sidecar found one P3 wording issue where the follow-up was folded into an older benchmark-backed audit sentence; the wording was split so this slice is documented as no-benchmark rejection-path evidence.
