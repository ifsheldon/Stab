# RPF3 Sweep And Gate Progress Report

## Implemented Slice: `detect` Default-False Sweep Sampling

Stab now permits `detect` sampling to execute selected sweep-conditioned circuits by using omitted all-false sweep bits for the current non-frame sampler, detector-conversion subset, and frame-path Pauli-observable subset.

The implemented boundary is deliberately narrow:

- `sample_detection_events` and `try_for_each_sampled_detection_event` compile non-frame circuits with sweep support and feed the detection converter an all-false sweep record.
- The frame-path detector sampler treats sweep controls on `CX` and `CY` qubit targets as false no-ops when `detect` has no sweep input, and follows Stim's `CZ` bit-target semantics by treating sweep/qubit pairs as false no-ops and bit/bit pairs as no-ops.
- `stab detect` accepts the same non-frame sweep-conditioned circuit shape through the public CLI.
- `stab detect` also accepts selected frame-path Pauli-observable circuits with sweep-controlled `CX`, `CY`, and `CZ` gates plus `CZ` bit/bit no-op groups under the same omitted all-false semantics.
- This slice does not add a Stab-specific sweep-input extension to `detect`, claim full sweep target-shape parity, or close analyzer sweep behavior beyond the selected no-op matrix and invalid target-position rejections. Pinned Stim v1.16.0 has no `stim detect --sweep` flag; typed detector-sampler sweep APIs are deferred to future Python or explicit Rust API work.

## Evidence

Oracle rows:

- `pf3-detect-default-false-sweep-core` runs `cargo test -p stab-core detection_sampling_uses_all_false_default_sweep_bits`.
- `pf3-detect-default-false-sweep-cli` runs `cargo test -p stab-cli detect_accepts_default_false`, covering both non-frame and selected frame-path CLI cases.
- `pf3-analyze-errors-sweep-core` runs `cargo test -p stab-core --test dem_analyzer_classical sweep`, covering selected analyzer sweep no-op and invalid target-position cases.
- `pf3-analyze-errors-sweep-cli` runs `cargo test -p stab-cli analyze_errors_sweep_controls`, covering public `stab analyze_errors` stdout, stderr, and exit-status behavior for the selected analyzer sweep matrix.
- `pf3-gate-semantic-wide-rust` runs `cargo test -p stab-core --test gate_semantic_execution`.

Direct tests:

- `detection_sampling_uses_all_false_default_sweep_bits` compares materialized and streaming non-frame detection sampling against an equivalent explicit all-false circuit.
- `detection_sampling_uses_all_false_default_sweep_bits_frame_path` compares materialized and streaming frame-path Pauli-observable detection sampling against an equivalent explicit all-false circuit, including repeated sweep controls and `CZ` bit/bit no-op groups.
- `detection_conversion_rejects_bad_sweep_records_and_unsupported_sampling_surfaces` continues to validate unsupported sweep target shapes in converter and frame contexts, including preflight validation of frame-path controlled-Pauli target shapes.
- `detect_accepts_default_false_sweep_conditioned_sampling` proves the public CLI accepts omitted all-false sweep sampling for a non-frame circuit.
- `detect_accepts_default_false_frame_path_sweep_conditioned_sampling` proves the public CLI accepts omitted all-false sweep sampling for a frame-path Pauli-observable circuit.
- `detect_rejects_invalid_frame_path_sweep_targets_before_opening_output` proves invalid frame-path sweep targets fail before `stab detect --out` opens or truncates the output path.
- `dem_analyzer_ignores_sweep_controls_like_upstream` ports the pinned Stim v1.16.0 `ErrorAnalyzer, ignores_sweep_controls` case and extends it to selected `CY`, `CZ`, `XCZ`, and `YCZ` sweep-control no-op cases, including `CZ` sweep/sweep and record/sweep bit-bit no-op groups.
- `dem_analyzer_rejects_invalid_sweep_target_positions` proves invalid controlled-Pauli sweep target positions fail with explicit analyzer errors instead of being silently ignored.
- `analyze_errors_sweep_controls_match_pf3_oracle` and `analyze_errors_sweep_controls_reject_invalid_target_positions` prove the public CLI returns the selected no-op DEM for the same matrix, empty stderr on success, empty stdout on failure, nonzero failure status, and actionable stderr.
- `fixed_tableau_gates_execute_across_current_public_surfaces` proves all 46 fixed-tableau gates execute through inverse-canceling sampler, detection-conversion, and analyzer circuits.
- `variable_target_spp_execution_matches_decomposed_circuit` proves supported Hermitian `SPP` and `SPP_DAG` products execute in the sampler and detection-conversion path with seeded output matching the public decomposed circuit.
- `variable_target_spp_executes_in_frame_detection_path` proves the detector-frame sampling path accepts an SPP circuit that requires frame sampling because it contains a Pauli observable target.
- `anti_hermitian_spp_execution_is_rejected_by_sampler_and_detection_conversion` proves anti-Hermitian `SPP` and `SPP_DAG` products still fail closed in sampler and detection-conversion compilation.
- `dem_analyzer_keeps_spp_explicitly_rejected_until_state_support_lands` proves analyzer support remains an explicit future analyzer-state-propagation task instead of an accidental acceptance.

Benchmark row:

- `pf3-m2d-sweep-b8` now has a non-primary report-only public CLI runner named `stab_pf3_m2d_sweep_b8`, normalized as shots per second, using the source-owned M9 packed sweep fixture.
- `pf3-m2d-sweep-ptb64-input` now has a non-primary report-only public CLI runner named `stab_pf3_m2d_sweep_ptb64`, normalized as shots per second, using deterministic source-owned `ptb64` measurement and sweep records generated by the benchmark harness.
- `pf3-detect-sweep-sampling` now has non-primary report-only Rust runners named `stab_detect_sweep_default_false` and `stab_detect_frame_sweep_default_false`, both normalized as shots per second.
- `pf3-analyze-errors-sweep` now has a non-primary report-only Rust runner named `stab_analyze_errors_sweep_control`, normalized as circuits per second over the selected analyzer sweep-control matrix.
- `pf3-gate-semantic-wide` now has a non-primary report-only Rust runner named `stab_pf3_gate_semantic_contract`, normalized as surface checks per second across fixed-tableau sampler compilation, detection-conversion compilation, analyzer propagation, and promoted `SPP`/`SPP_DAG` sampler plus detection-conversion checks.
- Earlier local probe command `just bench::compare --only pf3-m2d-sweep-b8 --only pf3-detect-sweep-sampling --only pf3-analyze-errors-sweep --baseline target/benchmarks/pf3-sweep-analyzer-probe-baseline/baseline.json --report target/benchmarks/pf3-sweep-analyzer-probe-compare` measured `stab_pf3_m2d_sweep_b8=0.000062208s`, or approximately `8.038e4 shots/s`, `stab_detect_sweep_default_false=0.000095184s`, or approximately `1.076e7 shots/s`, and `stab_analyze_errors_sweep_control=0.000001184s`, or approximately `8.446e5 circuits/s`, as report-only evidence on the local machine. The `ptb64` row has runner coverage but no fresh timing probe recorded in this report yet.
- Fresh local probe command `just bench::compare --only pf3-detect-sweep-sampling --baseline target/benchmarks/rpf3-detect-sweep-frame-probe/baseline.json --report target/benchmarks/rpf3-detect-sweep-frame-compare` measured `stab_detect_sweep_default_false=0.000093504s`, or approximately `1.095e7 shots/s`, and `stab_detect_frame_sweep_default_false=0.000263568s`, or approximately `3.885e6 shots/s`, as report-only evidence on the local machine.
- Fresh local probe command `just bench::compare --only pf3-analyze-errors-sweep --baseline target/benchmarks/rpf3-analyze-sweep-matrix-probe/baseline.json --report target/benchmarks/rpf3-analyze-sweep-matrix-compare` measured `stab_analyze_errors_sweep_control=0.000001872s`, or approximately `5.342e5 circuits/s`, as report-only evidence for the selected analyzer sweep-control matrix on the local machine.

## Still Open In RPF3

- Broader `pf3-analyze-errors-sweep` coverage remains open for analyzer sweep behavior beyond the selected sweep-control no-op matrix and invalid target-position rejections.
- Broader legal-gate execution remains open for analyzer `SPP`/`SPP_DAG` state propagation, remaining non-tableau legal operations, and future execution surfaces; the fixed-tableau gate contract and sampler/detection SPP subset are now covered.
- Broader frame-path sweep behavior remains open for unsupported sweep target placements. Any `detect --sweep` surface would be a Stab extension or future API decision, not a pinned Stim v1.16.0 CLI parity gap.

## Audit And Review Notes

Milestone audit found the slice complete against the selected PFM3 frame-path `detect` contract after keeping broader analyzer sweep behavior and full sweep target-shape parity explicitly open; later scope review confirmed `detect --sweep` is not a pinned Stim v1.16.0 CLI parity target.
The GPT-5.5/xhigh docs and metadata sidecar found stale roadmap wording that still described frame-path sweep sampling as open; this report and `rust-stim-drop-in-rewrite.md` now identify the selected frame-path subset as implemented.
The GPT-5.5/xhigh core sidecar found that unsupported frame-path sweep target shapes passed preflight validation and that `CZ` bit/bit groups were incorrectly treated as rejections instead of Stim-compatible no-ops; validation now rejects unsupported `CX` and `CY` bit targets before output files are opened, and `CZ` bit/bit groups are accepted as no-ops.
The follow-up analyzer audit found the selected sweep-control matrix complete after adding public `stab analyze_errors` CLI evidence. The GPT-5.5/xhigh analyzer sidecar found no correctness issues and suggested extra classical-bit edge cases, which are now covered. The GPT-5.5/xhigh docs and metadata sidecar found stale report boundary text and missing public CLI evidence; the report boundary is reworded and `pf3-analyze-errors-sweep-cli` now proves stdout, stderr, and exit-status behavior.
