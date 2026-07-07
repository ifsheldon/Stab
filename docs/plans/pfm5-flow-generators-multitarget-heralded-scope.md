# PFM5 Multi-Target Heralded Flow-Generator Scope

## Summary

This scope note locks a narrow PFM5 evidence slice for Rust `circuit_flow_generators` on selected multi-target heralded record-producing noise before Pauli-product measurement.
It does not claim full noisy-flow generator parity.

## Selected Surface

- Public API: `stab_core::circuit_flow_generators`.
- Input family: `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` with plain qubit targets followed by `MPP`.
- Comparator: exact ordered `Flow::to_string` output against pinned Stim v1.16.0 plus source-owned checker satisfaction with `check_if_circuit_has_unsigned_stabilizer_flows`.

## Selected Positive Cases

- `HERALDED_ERASE(0.04) 0 2` followed by `MPP X0*X1*Z2`.
- `HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 0 2` followed by `MPP X0*Y1*Z2`.
- `HERALDED_ERASE(0.04) 0 2` and `HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1 2` followed by `MPP X0*Y1*Z2`.

## Evidence

- `circuit_flow_generators_measurement_promotes_multi_target_heralded_noise_mpp_subset` records exact pinned Stim v1.16.0 generator strings for the selected cases.
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker` includes the combined multi-target case and proves generated flows satisfy the current unsigned checker subset.
- Oracle row `pf5-flow-generators-measurement-rust` selects the `measurement` test filter that includes the new exact case.
- Benchmark row `pf5-flow-generators-measurement-rich` now includes the combined multi-target heralded case in its report-only corpus.

## Explicit Non-Goals

- This slice does not select non-plain heralded targets, inverted heralded targets, unsupported classical heralded target shapes, broad noisy-flow checker semantics, or full noisy generator-table synthesis.
- This slice does not change Python binding behavior, CLI behavior, transform APIs, or public flow diagnostics.
- Broader heralded-noise generator synthesis remains under-specified until a future plan names exact circuits, comparator behavior, negative cases, resource behavior, oracle metadata, and benchmark policy.

## Required Commands

- `cargo test -p stab-core --test circuit_flow_generators heralded --quiet`
- `cargo test -p stab-core --test circuit_flow_generators measurement --quiet`
- `cargo test -p stab-bench pf5 --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list --milestone PF5`
- `just bench::smoke`
- `just bench::baseline --only pf5-flow-generators-measurement-rich --out target/benchmarks/pfm5-multitarget-heralded-flow-baseline`
- `just bench::compare --only pf5-flow-generators-measurement-rich --baseline target/benchmarks/pfm5-multitarget-heralded-flow-baseline/baseline.json --report target/benchmarks/pfm5-multitarget-heralded-flow-compare`
