# PFM3 Stochastic MPP Sampler And Detection Scope

## Summary

This note selects one exact PFM3 legal non-tableau execution subcase: `MPP(p)` stochastic Pauli-product measurement-result flip probabilities in the Rust sampler and detector-sampling surfaces.
The goal is to prove the already implemented sampler and detection paths treat `MPP(p)` as probabilistic measurement-record flips for selected deterministic Pauli-product bases, while keeping exact random-stream parity, broader stochastic non-tableau execution, and public interactive simulator APIs out of scope.

## Owned Surface

- Core sampler surface: `CompiledSampler` and seeded sample helpers for selected deterministic-base `MPP(p)` products.
- Core detection-conversion surface: `CompiledDetectionConverter` reference mapping for records produced by selected deterministic-base `MPP(p)` products.
- Core detection-sampling surface: `sample_detection_events` for non-frame and frame-path circuits whose detector or observable outputs depend on selected `MPP(p)` records.
- Existing analyzer dependency: selected analyzer `MPP(p)` detector-error propagation remains covered by `dem_analyzer_mpp_noise_and_result_flip_match_upstream_subset`; this slice does not change analyzer behavior.
- Checklist row: gate semantic execution.
- Active plan row: PFM3 legal non-tableau execution.
- Comparator class: statistical semantic parity for stochastic record flips plus exact reference-record mapping for conversion.
- Oracle row: `pf3-gate-mpp-stochastic-rust`.
- Benchmark row: none. This is semantic evidence for the existing Pauli-product measurement-flip path and does not add a distinct hot-path workload beyond existing sampler and detector-sampling rows.

## Selected Positive Cases

- `X 1; MPP(0.25) Z0 Z1` sampled through `CompiledSampler` produces the expected four-bucket distribution for two independently sampled Pauli-product measurement records with deterministic bases `0` and `1`; output buckets `00`, `01`, `10`, and `11` are expected near probabilities `0.1875`, `0.5625`, `0.0625`, and `0.1875`.
- `CompiledDetectionConverter` maps unflipped and flipped selected `MPP(p)` records against the deterministic reference sample without treating the probability argument as a layout change.
- Non-frame `sample_detection_events` propagates selected stochastic `MPP(p)` records into detector and observable outputs sourced from the same record.
- Frame-path `sample_detection_events` propagates selected stochastic `MPP(p)` records into record-backed observable outputs without introducing unrelated Pauli-frame observable flips.

## Explicit Rejections And Non-Goals

- This slice does not require exact C++ Stim random streams.
- This slice does not add public interactive simulator APIs, Python detector-sampler APIs, or exact CLI byte-stream fixture parity for random output.
- This slice does not promote broader stochastic `MPP(p)` behavior outside the selected deterministic-base sampler and detector-sampling paths.
- This slice does not promote stochastic `MPAD(p)` behavior; that is owned separately by `docs/plans/pfm3-stochastic-mpad-sampler-detection-scope.md`.
- This slice does not change analyzer behavior; selected analyzer `MPP(p)` exact-output evidence remains owned by existing M10 and PF3 analyzer tests.

## Tests

- Add `stochastic_mpp_executes_across_sampler_and_detection_surfaces` in `crates/stab-core/tests/gate_semantic_execution.rs`.
- The test must check sampler bucket statistics, converter reference behavior, non-frame detection statistics, and frame-path detection statistics.
- The sampler check uses 4000 shots with a fixed seed and derived 5-sigma binomial windows for the selected four buckets.
- The non-frame and frame-path detection-sampling checks use 4000 shots with fixed seeds and derived 5-sigma binomial windows for probability `0.25`.
- The statistical checks use deterministic seeds and 5-sigma count windows to prove semantic distribution without depending on exact C++ random streams; the aggregate false-rejection risk for the selected checks is below `0.001` under the binomial normal approximation.

## Done Criteria

- The focused Rust test passes without weakening deterministic `MPP`, stochastic `MPAD(p)`, or analyzer `MPP(p)` coverage.
- `oracle/fixtures/manifest.csv` includes `pf3-gate-mpp-stochastic-rust` as implemented statistical parity evidence with a structural Rust runner.
- `docs/plans/pfm3-gate-semantic-boundary-scope.md`, `docs/plans/rpf3-sweep-gate-progress-report.md`, `docs/plans/non-deferred-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, `docs/stab-feature-checklist.md`, and `docs/plans/milestone-spec-gaps.md` describe the selected stochastic sampler and detection evidence without claiming full legal non-tableau parity.
- `cargo test -p stab-core --test gate_semantic_execution stochastic_mpp --quiet`, `cargo test -p stab-core --test gate_semantic_execution mpp --quiet`, `cargo test -p stab-oracle fixtures --quiet`, `just oracle::run --milestone PF3 --structural`, and `just bench::smoke` pass before commit.
