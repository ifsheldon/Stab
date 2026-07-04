# RPF3 Sweep And Gate Progress Report

## Implemented Slice: Non-Frame `detect` Default-False Sweep Sampling

Stab now permits non-frame `detect` sampling to execute sweep-conditioned circuits by using omitted all-false sweep bits for the current sampler and detector-conversion subset.

The implemented boundary is deliberately narrow:

- `sample_detection_events` and `try_for_each_sampled_detection_event` compile non-frame circuits with sweep support and feed the detection converter an all-false sweep record.
- `stab detect` accepts the same non-frame sweep-conditioned circuit shape through the public CLI.
- Frame-path Pauli-observable detection sampling still rejects sweep-conditioned circuits with the existing explicit sweep-support error.
- This slice does not add typed sweep input files to `detect`, expand analyzer sweep behavior, or claim full sweep target-shape parity.

## Evidence

Oracle rows:

- `pf3-detect-default-false-sweep-core` runs `cargo test -p stab-core detection_sampling_uses_all_false_default_sweep_bits`.
- `pf3-detect-default-false-sweep-cli` runs `cargo test -p stab-cli detect_accepts_default_false_sweep_conditioned_sampling`.

Direct tests:

- `detection_sampling_uses_all_false_default_sweep_bits` compares materialized and streaming non-frame detection sampling against an equivalent explicit all-false circuit.
- `detection_conversion_rejects_bad_sweep_records_and_unsupported_sampling_surfaces` now also validates that frame-path sweep-conditioned Pauli-observable detection sampling remains rejected.
- `detect_accepts_default_false_sweep_conditioned_sampling` proves the public CLI accepts omitted all-false sweep sampling for a non-frame circuit.
- `detect_still_rejects_frame_path_sweep_conditioned_sampling` proves the public CLI keeps the frame-path rejection.

Benchmark row:

- `pf3-detect-sweep-sampling` now has a non-primary report-only Rust runner named `stab_detect_sweep_default_false`, normalized as shots per second.
- Local probe command `just bench::compare --only pf3-detect-sweep-sampling --baseline target/benchmarks/pf3-detect-sweep-probe/baseline.json --report target/benchmarks/pf3-detect-sweep-compare` measured `stab_detect_sweep_default_false=0.000090864s`, or approximately `1.127e7 shots/s`, as report-only evidence on the local machine.

## Still Open In RPF3

- `pf3-m2d-sweep-b8` and `pf3-m2d-sweep-ptb64-input` still need benchmark runner extraction if their corresponding packed and transposed sweep paths are promoted.
- `pf3-analyze-errors-sweep` remains open for analyzer sweep behavior.
- `pf3-gate-semantic-wide` remains open for systematic legal-gate execution classification across sampler, converter, detection, and analyzer paths.
- Frame-path sweep-conditioned detector sampling remains unsupported until the frame executor owns typed sweep semantics.
