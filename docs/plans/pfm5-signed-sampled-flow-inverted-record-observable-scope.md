# PFM5 Signed Sampled Flow Inverted Record Observable Scope

## Summary

This note locks the PF5 signed sampled-flow evidence for the pinned Stim v1.16.0 inverted record-backed observable case.
The owned subcase is `sample_if_circuit_has_stabilizer_flows_inverted_obs_rec` from `vendor/stim/src/stim/util_top/has_flow.test.cc`, translated to the Rust API `sample_if_circuit_has_stabilizer_flows`.

## Owned Behavior

- Circuit: one inverted Z-basis measurement `M !0` followed by `OBSERVABLE_INCLUDE(3) rec[-1]`.
- Positive flow: `-Z0 -> obs[3]`.
- Negative flow: `Z0 -> obs[3]`.
- Comparator: structural boolean result parity against the pinned Stim expectation `[true, false]`, with a fixed Stab seed because exact random streams are not a compatibility target.
- Public surface: Rust `stab-core` signed sampled-flow checker only.
- Resource behavior: the existing sampled-flow checker sample-count rounding and augmentation resource limits apply; this subcase does not add a new public IO path or materialization surface.

## Explicit Non-Goals

- Python `Circuit.has_flow`, `Circuit.has_all_flows`, or binding ergonomics.
- Exact sampled random-stream parity with Stim.
- New signed sampled-flow diagnostics.
- Broader observable provenance, `stim explain_errors`, or ErrorMatcher stack-frame parity.
- New benchmark coverage, because the subcase reuses the existing sampled-flow checker path and is not a distinct throughput workload.

## Evidence

- `sample_if_circuit_has_stabilizer_flows_checks_inverted_record_observables` in `crates/stab-core/tests/circuit_flows.rs`.
- Oracle row `pf5-signed-sampled-flows-rust` selects `cargo test -p stab-core --test circuit_flows sample_if_circuit_has_stabilizer_flows`.
- Existing report-only benchmark row `pf5-has-all-flows-batch` remains scoped to unsigned has-all helpers and is not affected by this signed sampled-flow evidence.

## Verification

```sh
cargo test -p stab-core --test circuit_flows sample_if_circuit_has_stabilizer_flows --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
just bench::smoke
```
