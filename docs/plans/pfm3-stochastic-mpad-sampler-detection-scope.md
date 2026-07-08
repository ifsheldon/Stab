# PFM3 Stochastic MPAD Sampler And Detection Scope

## Summary

This note selects one exact PFM3 legal non-tableau execution subcase: `MPAD(p)` stochastic measurement-pad flip probabilities in the Rust sampler and detector-sampling surfaces.
The goal is to prove the already implemented sampler and detection paths treat `MPAD(p)` as probabilistic measurement-record flips instead of deterministic pad records, while keeping exact random-stream parity and broader stochastic non-tableau execution out of scope.

## Owned Surface

- Core sampler surface: `CompiledSampler` and seeded sample helpers for `MPAD(p)`.
- Core detection-conversion surface: `CompiledDetectionConverter` reference mapping for records produced by `MPAD(p)`.
- Core detection-sampling surface: `sample_detection_events` for non-frame and frame-path circuits whose detector or observable outputs depend on `MPAD(p)` records.
- Checklist row: gate semantic execution.
- Active plan row: PFM3 legal non-tableau execution.
- Comparator class: statistical semantic parity for stochastic record flips plus exact reference-record mapping for conversion.
- Oracle row: `pf3-gate-mpad-stochastic-rust`.
- Benchmark row: none. This is semantic evidence for the existing measurement-flip path and does not add a distinct hot-path workload beyond existing sampler and detector-sampling rows.

## Selected Positive Cases

- `MPAD(0.25) 0 1` sampled through `CompiledSampler` produces the expected four-bucket distribution for two independently sampled pad records, with output buckets `00`, `01`, `10`, and `11` expected near probabilities `0.1875`, `0.5625`, `0.0625`, and `0.1875`.
- `CompiledDetectionConverter` maps unflipped and flipped `MPAD(p)` records against the deterministic reference sample without treating the probability argument as a layout change.
- Non-frame `sample_detection_events` propagates stochastic `MPAD(p)` records into detector and observable outputs sourced from the same record.
- Frame-path `sample_detection_events` propagates stochastic `MPAD(p)` records into record-backed observable outputs without introducing unrelated Pauli-frame observable flips.

## Explicit Rejections And Non-Goals

- This slice does not require exact C++ Stim random streams.
- This slice does not add public interactive simulator APIs, Python detector-sampler APIs, or exact CLI byte-stream fixture parity for random output.
- This slice does not promote broader stochastic `MPP(p)`, broader stochastic `MPAD(p)` behavior outside the selected sampler and detector-sampling paths, or future execution surfaces.
- This slice does not change analyzer behavior; selected analyzer `MPAD(p)` exact-output evidence remains owned by `docs/plans/pfm3-mpad-noisy-analyzer-scope.md`.

## Tests

- Add `stochastic_mpad_executes_across_sampler_and_detection_surfaces` in `crates/stab-core/tests/gate_semantic_execution.rs`.
- The test must check sampler statistics, converter reference behavior, non-frame detection statistics, and frame-path detection statistics.
- The sampler check uses 4000 shots with seed 17 and derived 5-sigma binomial windows: `00` and `11` must land in `626..=874`, `01` must land in `2093..=2407`, and `10` must land in `173..=327`.
- The non-frame detection-sampling check uses 4000 shots with seed 23 and requires the single stochastic detector count to land in the derived 5-sigma `863..=1137` window, with the observable count exactly matching the detector count because both are sourced by the same measurement record.
- The frame-path detection-sampling check uses 4000 shots with seed 29 and requires the record-backed observable count to land in the derived 5-sigma `863..=1137` window, while the unrelated Pauli-backed observable count must remain zero.
- The statistical checks use deterministic seeds and 5-sigma count windows to prove semantic distribution without depending on exact C++ random streams; the aggregate false-rejection risk for the selected checks is below `0.001` under the binomial normal approximation.

## Done Criteria

- The focused Rust test passes without weakening deterministic `MPAD` or analyzer `MPAD(p)` coverage.
- `oracle/fixtures/manifest.csv` includes `pf3-gate-mpad-stochastic-rust` as implemented structural evidence.
- `docs/plans/pfm3-gate-semantic-boundary-scope.md`, `docs/plans/rpf3-sweep-gate-progress-report.md`, `docs/plans/non-deferred-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, `docs/stab-feature-checklist.md`, and `docs/plans/milestone-spec-gaps.md` describe the selected stochastic sampler and detection evidence without claiming full legal non-tableau parity.
- `cargo test -p stab-core --test gate_semantic_execution stochastic_mpad --quiet`, `cargo test -p stab-core --test gate_semantic_execution mpad --quiet`, `cargo test -p stab-oracle fixtures --quiet`, `just oracle::run --milestone PF3 --structural`, and `just bench::smoke` pass before commit.
