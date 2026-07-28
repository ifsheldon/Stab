# Goal: Build Detection And DEM Batch Pipelines

## Objective

Finish milestone A5 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): give measurement-to-detection conversion, circuit detection sampling, and DEM sampling the same compiler, immutable-plan, mutable-session, typed-batch, and sink architecture established for circuit sampling.

## Current State

- A0 through A4 are complete.
- `stab-bits` and `stab-records` are physical Stable Rust 1.97.1 crates.
- Circuit sampling uses the public compiler, plan, session, and sink path; `CompiledSampler` remains only a compatibility adapter.
- Detection conversion and DEM sampling still have older materialized or command-specific centers that A5 must replace.
- A5 keeps detector-frame execution and the detector-only and sampled-error DEM algorithms as distinct private variants because their work and random consumption differ.
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

1. Inventory the existing detection converter, detector sampler, DEM sampler, replay, CLI, oracle, and benchmark call paths; freeze exact compatibility and resource behavior before moving code.
2. Introduce three operation-specific compiler, immutable-plan, mutable-session, request, summary, cancellation, and error families without a universal execution trait or public executable IR.
3. Add typed measurement-to-detection composition and distinct detector, observable, and sampled-error batch planes using `stab-records` sinks.
4. Migrate `detect`, `m2d`, and `sample_dem`, retaining command-wide path preflight, replay validation before output activation, valid-prefix `m2d` output, writer-error propagation, and existing Stim-compatible bytes.
5. Port materialized conveniences to thin adapters, regenerate correctness and performance ownership, and add phase-separated benchmarks plus affected process-symmetric CLI comparisons.
6. Run milestone-audit and full-code-review, fix every confirmed finding, then commit source, CLI, tests, benchmark contracts, and documentation in focused commits.
7. Produce clean source-current evidence with unique artifact paths and close A5 only when every comparable row passes the unchanged `1.25x` Stim gate and every new unlike measurement is explicitly unseeded for Stab self-regression.

## Nonnegotiable Contracts

- Execution imports no text codec, filesystem, CLI, or ops API.
- The three operation families remain distinct; no generic plan/session abstraction is introduced without two proven implementations and a real caller need.
- Plans are immutable and shareable; sessions own reusable mutable state and poison after execution or sink failure.
- Batch sizes are bounded implementation details and cannot change semantic output.
- Conversion and reference-sample scratch scale with width and active batch size, not total shots or input record count.
- `m2d` consumes initial input record-at-a-time so a later malformed record cannot suppress already committed valid-prefix output.
- Replay input is validated and rewound through the retained preflight handle before any output sink can create or truncate a file.
- Detector-only and sampled-error DEM paths stay separate and preserve their established random-consumption semantics.
- Cancellation occurs only at documented batch or record boundaries and preserves exact progress.
- Existing `.stim`, `.dem`, result-format, CLI, seeded Stab, and statistical Stim contracts do not change.
- New phase identities remain report-only and unseeded unless an exact prior identity exists; comparable process rows keep the `1.25x` Stim parity gate.

## Done Criteria

- Every `detect`, `m2d`, and `sample_dem` product path delegates to its public compiler, plan, session, and sink architecture.
- Streamed and materialized results agree across formats, reference modes, sweep-conditioned conversion, observable routing, correlated DEM events, replay, and sampled-error output.
- Same-session partitioning, cancellation, poisoning, valid-prefix delivery, replay-before-output safety, writer failures, path aliases, and bounded allocation have direct tests.
- Qualification inventories regenerate exactly and no implemented A5 behavior has only planned ownership.
- Clean phase and affected CLI benchmarks show no unexplained material regression.
- Milestone-audit, full-code-review, workspace verification, architecture enforcement, implemented oracles, qualification checks, benchmark smoke, and pre-commit have no open A5 finding.

## Required Checks

Use targeted tests during implementation. Before each focused commit, run the checks for touched crates and staged pre-commit validation. Before A5 closure, run formatting, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture enforcement, implemented and result-format oracles, correctness and performance check/regeneration, generated-status checking, benchmark smoke, and clean revision-named phase and comparable CLI evidence.
