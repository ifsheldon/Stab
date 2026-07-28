# Goal: Build Detection And DEM Batch Pipelines

## Objective

Finish milestone A5 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): give measurement-to-detection conversion, circuit detection sampling, and DEM sampling the same compiler, immutable-plan, mutable-session, typed-batch, and sink architecture established for circuit sampling.

## Current State

- A0 through A4 are complete.
- `stab-bits` and `stab-records` are physical Stable Rust 1.97.1 crates.
- Circuit sampling uses the public compiler, plan, session, and sink path; `CompiledSampler` remains only a compatibility adapter.
- A5 implementation is present: measurement conversion, detection sampling, and DEM sampling expose separate compiler, immutable-plan, mutable-session, typed-batch, cancellation, progress, and sink families through `stab_core::execution`.
- `detect`, `m2d`, and `sample_dem` use those public execution seams. Finite-shot sampling materializers and visitors delegate through compatibility adapters; the public per-record detection converter and unknown-length DEM replay iterator remain explicit low-level compatibility kernels.
- Direct detector-frame and fused sample-convert execution remain distinct private variants. Detector-only, sampled-error, and replay DEM execution also remain distinct because their work and random consumption differ.
- The first milestone audit findings are fixed. The first full-code-review then found five additional contract defects: incremental conversion could split one sink lifecycle, direct detector-frame compilation did not charge its retained executable circuit, the replay convenience API scanned records before admission, process-symmetric rows validated only Stab output, and report-only phases did not reject changed witnesses.
- Those five review findings are repaired in the working tree with direct tests and regenerated correctness and performance inventories. Focused commits, a second audit and review, source-current clean evidence, and closure synchronization remain.
- Physical extraction of the remaining model, algebra, analysis, engine, facade, and SIMD components belongs to A6, after A5 proves these execution boundaries inside `stab-core`.

## Sources Of Truth

- Active milestone: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), A5
- Component boundaries: [../architecture/component-contracts.md](../architecture/component-contracts.md)
- Dependency graph: [../architecture/README.md](../architecture/README.md)
- API migration inventory: [../architecture/0.2-api-migration-inventory.md](../architecture/0.2-api-migration-inventory.md)
- Progress record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Specification gaps: [milestone-spec-gaps.md](milestone-spec-gaps.md)

Stop and repair the owning source when code, tests, generated inventories, benchmark contracts, or these documents disagree.

## Execution Sequence

1. Finish focused verification of the five full-code-review repairs and commit core, CLI, benchmark, qualification, and documentation changes separately.
2. Run milestone-audit and full-code-review again against the repaired clean revision; fix every confirmed finding and amend only genuine specification gaps.
3. From the resulting clean revision, run source-current A5 phase diagnostics and all affected comparable CLI rows into new artifact paths.
4. Keep unlike phase identities report-only and unseeded. Require every comparable process row to pass the unchanged `1.25x` Stim gate without waivers, with independent untimed Stim and Stab output witnesses.
5. Run the complete required checks, synchronize the progress report and generated dashboard, commit closure documentation, and hand A6 the physical crate extraction.

## Nonnegotiable Contracts

- Execution imports no text codec, filesystem, CLI, or ops API.
- The three operation families remain distinct; no generic plan/session abstraction is introduced without two proven implementations and a real caller need.
- Plans are immutable and shareable; sessions own reusable mutable state and poison after execution or sink failure.
- Batch sizes are bounded implementation details and cannot change semantic output.
- Conversion and reference-sample scratch scale with width and active batch size, not total shots or input record count.
- DEM caller byte limits cover the active reusable record, error, packed-plane, and compatibility-sink storage; fused detection applies the private session limit to the combined sampling and conversion estimate.
- The DEM byte policy covers width-dependent heap storage and compatibility record containers. Immutable plans, caller-owned returned materializations, RNG state, and fixed session metadata are outside that dynamic scratch budget.
- `m2d` consumes initial input record-at-a-time so a later malformed record cannot suppress already committed valid-prefix output.
- One `m2d` conversion delivery remains bound to one sink until exactly one finish; double finish, write-after-finish, finish failure, and abandoned committed output have explicit progress and poison semantics.
- Replay input is validated and rewound through the retained preflight handle before any output sink can create or truncate a file.
- DEM replay rejects poison state and total traversal work before scanning caller-owned record widths.
- Direct detector-frame compilation charges the complete retained conversion and executable-circuit representation before materialization.
- Detector-only and sampled-error DEM paths stay separate and preserve their established random-consumption semantics.
- Cancellation occurs only at documented batch or record boundaries and preserves exact progress.
- Existing `.stim`, `.dem`, result-format, CLI, seeded Stab, and statistical Stim contracts do not change.
- New phase identities remain report-only and unseeded unless an exact prior identity exists; comparable process rows keep the `1.25x` Stim parity gate.
- Every process-symmetric A5 row validates independent pinned-Stim and Stab output witnesses outside timing. Every report-only compile phase validates exact source-owned plan dimensions or a frozen plan fingerprint, and every output-producing phase validates shot counts plus a frozen result or ordered sequence digest.

## Done Criteria

- Every `detect`, `m2d`, and `sample_dem` product path delegates to its public compiler, plan, session, and sink architecture.
- Streamed and materialized results agree across formats, reference modes, sweep-conditioned conversion, observable routing, correlated DEM events, replay, and sampled-error output; a 4,096-record matrix crosses multiple batches for every supported command format and side-output route.
- Same-session partitioning, cancellation including replay finish-time cancellation, poisoning, valid-prefix delivery and progress, replay-before-output safety, writer failures, path aliases, caller byte admission, aggregate fused-session admission, and bounded allocation have direct tests.
- Qualification inventories regenerate exactly and no implemented A5 behavior has only planned ownership.
- Clean phase and affected CLI benchmarks show no unexplained material regression.
- Milestone-audit, full-code-review, workspace verification, architecture enforcement, implemented oracles, qualification checks, benchmark smoke, and pre-commit have no open A5 finding.

## Required Checks

Use targeted tests during implementation. Before each focused commit, run the checks for touched crates and staged pre-commit validation. Before A5 closure, run formatting, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture enforcement, implemented and result-format oracles, correctness and performance check/regeneration, generated-status checking, benchmark smoke, and clean revision-named phase and comparable CLI evidence.
