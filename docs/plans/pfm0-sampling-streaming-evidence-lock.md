# PFM0 Sampling And Streaming Evidence Lock

## Summary

This PFM0 reconciliation slice closes two checklist rows whose remaining gap wording was only about deferred class-shaped or Python API parity, not active Rust or CLI implementation work.
The rows are `Streaming DEM sampling` and `Detector sampling` in `docs/stab-feature-checklist.md`.
The closure does not change runtime behavior.
It tightens the status language so future agents do not reopen current Rust and CLI sampling surfaces when the missing work is already tracked by the deferred sampler and converter classes row.

## Closed Rows

| Checklist row | Current status | Closure boundary |
| --- | --- | --- |
| `Streaming DEM sampling` | Done for current Rust and CLI surface | Public Rust visitor APIs exist for seeded detector-event streaming, detector-event plus sampled-error streaming, and replayed sampled-error conversion. `stab sample_dem` uses streaming writers for detector output, observable side output, sampled-error output, replayed-error copying, and `ptb64` 64-record grouping. |
| `Detector sampling` | Done for selected Rust and CLI surface | Public Rust materialized and visitor functions exist through `sample_detection_events` and `try_for_each_sampled_detection_event`, and `stab detect` streams sampled detection records through CLI writers for the current supported detector sampling surface. |

## Evidence

Rust and CLI surfaces:

- `CompiledDemSampler::try_for_each_detection_event_with_seed` reuses one detection record, streams seeded detector and observable samples, and returns visitor errors promptly.
- `CompiledDemSampler::try_for_each_detection_event_and_error_with_seed` reuses one detection record and one sampled-error buffer for detector, observable, and sampled-error streaming.
- `CompiledDemSampler::try_for_each_detection_event_from_error_records` streams replayed error records without materializing every requested shot.
- `stab sample_dem` calls the streaming DEM sampler visitors and writes detector, observable, error, replay, and `ptb64` outputs through bounded writer paths.
- `try_for_each_sampled_detection_event` streams circuit detector samples for the selected non-frame and frame-path detection surfaces.
- `stab detect` writes sampled detection records through streaming detection writers.

Source-owned tests:

- `cargo test -p stab-core --test dem_sampler dem_sampler --quiet`
- `cargo test -p stab-cli sample_dem --quiet`
- `cargo test -p stab-core detection --quiet`
- `cargo test -p stab-cli detect --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone M11 --exact`
- `just oracle::run --milestone M9 --exact`
- `just oracle::run --milestone M9 --structural`
- `just oracle::run --milestone PF3 --structural`

Supporting reports:

- `docs/plans/post-beta-fix-report.md` records the additive DEM visitor APIs and streaming CLI writer migration.
- `docs/plans/m11-progress-report.md` records the `sample_dem` detector, observable, error, replay, format, and resource-boundary evidence.
- `docs/plans/m9-completion-report.md` records the base `detect` CLI detector sampling, output-format, oracle, and done-criteria evidence that PF3 later extends with default-false sweep behavior.
- `docs/plans/rpf3-sweep-gate-progress-report.md` records current detector sampling, selected default-false sweep behavior, and the boundary around absent `stim detect --sweep` parity.
- `docs/plans/rpf4-dem-sampler-progress-report.md` records folded DEM sampler compilation and direct detector sampling through repeats.

## Deferred Boundaries

The following surfaces remain intentionally outside this evidence lock:

- Python `CompiledDetectorSampler` and `CompiledDemSampler` class shapes, including Python `sample`, `sample_bit_packed`, and `sample_write` ergonomics.
- Any future explicit Rust `CompiledDetectorSampler` product that tries to mirror Stim's Python detector-sampler class instead of the current free-function and visitor API surface.
- A Stab-specific `detect --sweep` extension, because pinned Stim v1.16.0 has no `stim detect --sweep` flag.
- Broader sweep-conditioned analyzer behavior, broader legal-gate execution semantics, and broader sweep target-shape parity tracked by PFM3.
- Full folded traversal across graphlike, hypergraph, SAT, analyzer, matcher, and every DEM consumer tracked by PFM4.
- Sampled-error materialization and replay width-cap removal for DEM sampling, which remains separate from detector-only streaming output.

## Documentation Updates

This slice updates `docs/stab-feature-checklist.md` so `Streaming DEM sampling` and `Detector sampling` no longer appear as active non-deferred partial rows.
The deferred class-shaped API work remains visible in the `Sampler and converter classes` row.
The active plan and inventory now point to this evidence lock when explaining why these sampling rows should not be treated as open implementation milestones.

## Verification

Before committing this slice, run the targeted checks listed in the Evidence section plus:

```sh
just oracle::list
just oracle::matrix --check
just bench::list
cargo fmt --all --check
just maintenance::pre-commit
```

Milestone-audit should verify that this document does not close broader sweep, analyzer, folded traversal, Python, or future compiled detector-sampler product work.
Full-code-review should verify that the checklist wording does not overclaim API parity beyond the selected Rust and CLI surfaces.
