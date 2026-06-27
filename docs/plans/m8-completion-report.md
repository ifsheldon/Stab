# M8 Completion Report

## Milestone

M8: Circuit Sampling.

Objective: implement Stim's core circuit-sampling behavior with clear analysis-vs-shot separation and early bit-packed output support.

## Tests Ported Or Created

- Added and extended `crates/stab-core/src/sampling/tests.rs` for deterministic sampling, reset and measurement edge cases, measurement feedback, repeat blocks, reference-sample behavior, output-format padding, bit-packed output, Pauli noise, depolarizing noise, heralded local noise, Pauli-channel noise, and correlated-error branch semantics.
- Added exact oracle fixtures for deterministic `stim sample` output, including correlated errors and `--skip_loop_folding` on a repeat circuit.
- Added statistical oracle fixtures for noisy sampling with fixed seeds, sample counts, and tolerance metadata in `oracle/fixtures/manifest.csv`.
- Added bucketed statistical oracle rows for `PAULI_CHANNEL_2`, correlated errors, independent X/Y/Z errors, depolarizing basis variants, multi-target `X_ERROR`, and measurement-result flip probabilities.
- Added semantic-mining trace rows for Python compiled measurement sampler, frame simulator, and tableau simulator semantics that map to the current Rust sampler tests.

## Implementation Areas

- `crates/stab-core/src/sampling.rs` owns `CompiledSampler`, analysis-vs-shot separation, reference-sample construction, repeat execution, measurement/reset handling, feedback, output writers, and supported noisy-sampling operations.
- Correlated error support covers `E`, `CORRELATED_ERROR`, and `ELSE_CORRELATED_ERROR` with Stim-compatible hidden branch state inside each shot.
- Measurement result-flip probabilities are sampled for single-qubit, pair, Pauli-product, and `MPAD` measurements; anti-Hermitian MPP products are rejected during sampler compilation.
- `stab-cli sample` accepts the M8 core flags, input/output paths, measurement output formats, seed handling, `--skip_reference_sample`, and `--skip_loop_folding`.
- `ops/bench/src/baseline.rs` and `ops/bench/src/baseline/m8.rs` provide M8 Stab-side comparison runners and strict benchmark completeness checks.

## Oracle And Benchmark Evidence

- `just oracle::run --milestone M8 --exact` passes deterministic sampling rows.
- `just oracle::run --milestone M8 --statistical` passes noisy statistical rows, including the bucketed multi-outcome fixtures.
- `just oracle::run --milestone M8` passes all M8 oracle rows and semantic-mining trace rows.
- `just bench::compare --milestone M8 --strict` validates pinned Stim v1.16.0 baseline metadata, rejects selected pending rows, rejects missing or invalid selected baseline rows, rejects empty contract-only placeholders, and reports measured Stab-side runners for every M8 benchmark manifest row.
- M8 benchmark output includes compile/analysis time, one-shot latency, 1024-shot throughput, 1,000,000-shot throughput, frame/tableau sampler proxy timings, measurement reader timings, probability sampling timings, reference sample tree timing, and representative primary-matrix contract rows.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Deterministic sampling oracle rows pass | Satisfied | `just oracle::run --milestone M8 --exact` |
| Noisy statistical oracle rows pass | Satisfied | `just oracle::run --milestone M8 --statistical` |
| Core sampler tests cover repeat blocks, feedback, reset and measurement edge cases, and output-format padding | Satisfied | `cargo test -p stab-core sampling` |
| CLI sample tests cover public M8 flags and output formats | Satisfied | `cargo test -p stab-cli sample` |
| M8 strict benchmark compare has no pending, missing-baseline, invalid-baseline, metadata-mismatch, or empty contract-only rows | Satisfied | `just bench::compare --milestone M8 --strict` |

## Audit Outcome

Milestone audit found implementation and evidence issues in correlated-error sampling, multi-outcome statistical coverage, `--skip_loop_folding` evidence, Python semantic-mining traceability, strict benchmark completeness, pinned Stim baseline metadata validation, and durable benchmark evidence.
Implementation issues were fixed with correlated-error sampler support, exact and statistical oracle rows, semantic-mining trace rows, strict benchmark metadata and completeness checks, representative M8 contract runners, and this completion report.

Resolved M8 spec entries:

- `2026-06-27 - M8: Linked Simulator And Result-Format Subcase Ownership`
- `2026-06-27 - M8: Benchmark Strictness And Baseline Completeness`
- `2026-06-27 - M8: Multi-Outcome Statistical Evidence`

Open M8 spec entries:

- `2026-06-27 - M8: Skip Loop Folding Scope`

## Full Code Review Outcome

Full code review found issues in statistical false-positive budget enforcement, `circuit_vs_amplitudes` matrix ownership, strict benchmark placeholder-baseline acceptance, typoed benchmark milestone filters, anti-Hermitian MPP handling, noisy measurement probabilities, and CLI `--seed`/`--shots` integer bounds.
Those issues were fixed with oracle budget validation, M12 matrix ownership for amplitude cross-checks, stricter benchmark baseline validation, milestone-filter rejection, MPP phase validation, measurement-result flip sampling, CLI value parsers, and focused regression tests.

The review also raised heralded-noise sample-output emission, but direct verification against pinned Stim v1.16.0 showed `stim sample` does not emit herald bits as output bits for the M8 exact fixture; Stab keeps herald bits in the measurement record for feedback while preserving pinned Stim output compatibility.

## Verification Commands

- `cargo fmt --check`
- `cargo test -p stab-bench`
- `cargo test -p stab-core sampling`
- `cargo test -p stab-cli sample`
- `cargo test -p stab-oracle repository_fixture_manifest_passes_validation`
- `cargo test -p stab-oracle statistical`
- `just oracle::matrix --check`
- `just oracle::run --milestone M8`
- `just bench::compare --milestone M8 --strict`
