# Goal: Extract The Product Components

## Objective

Finish milestone A6 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): physically extract the remaining model, algebra, analysis, engine, and portable-SIMD boundaries while preserving the complete A0–A5 behavior and evidence contracts.

## Current State

- A0 through A5 are complete. A5 closes at clean source revision `b8e3f459d2a8817aa98ca0d71072a9529fa9fe9c`.
- The physical product crates are currently `stab-algebra`, `stab-analysis`, `stab-bits`, `stab-model`, `stab-records`, `stab-core`, and `stab-cli`.
- `stab-algebra`, `stab-analysis`, `stab-bits`, `stab-model`, and `stab-records` build on Stable Rust 1.97.1. `stab-model` physically owns the complete circuit and DEM compatibility models. The current `stab-analysis` slices physically own gate semantic projections, fixed-shape unitary metadata, gate and circuit tableau lowering, circuit simplification and decomposition, recursive circuit and DEM tag removal, bounded circuit and DEM flattening, noise removal, DEM probability rounding, SAT/WCNF materialization, graphlike and hypergraph logical-error search, repetition/surface/color generation, MBQC decomposition, unsigned flow checking/generation/solving, sparse reverse-frame tracking, unitary and selected QEC inversion, tracker-driven flow reversal, bounded feedback lowering, detecting regions, missing-detector analysis, circuit-to-DEM lowering and loop folding, XYZ error-probability decomposition, and typed analysis resource admission. `stab-core` preserves the established public facade and still owns matching and execution implementations; portable SIMD is temporarily absent until a dependency-free raw kernel owner restores a distinct measured implementation.
- `ops-contracts` is removed. Qualification policy is oracle-owned, and analyzer benchmarks derive compact-work witnesses from public DEM output instead of hidden product counters.
- Logical ownership, typed diagnostics, resource policies, fingerprints, capabilities, plans, sessions, batches, and sinks are already tested inside the current compilation boundary.
- A6 must still extract `stab-analysis`, `stab-engine`, and `stab-kernels-simd`, then curate `stab-core` as the final facade rather than another implementation owner.

## Sources Of Truth

- Active milestone: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), A6
- Product graph: [../architecture/README.md](../architecture/README.md)
- Exact extraction map: [../architecture/a6-component-extraction-map.md](../architecture/a6-component-extraction-map.md)
- Component contracts: [../architecture/component-contracts.md](../architecture/component-contracts.md)
- API migration inventory: [../architecture/0.2-api-migration-inventory.md](../architecture/0.2-api-migration-inventory.md)
- Progress record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Specification gaps: [milestone-spec-gaps.md](milestone-spec-gaps.md)

Stop and repair the owning source when Cargo metadata, architecture checks, public API inventory, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Freeze an exact module-to-crate move map, public replacement map, feature map, and dependency DAG before moving files.
2. Keep the extracted scalar `stab-algebra` green on Stable and add its external-consumer fixture with the consolidated A6 fixture matrix.
3. Completed: move circuit and DEM syntax, parsing, printing, fingerprints, tags, diagnostics, compact traversal, and resource vocabulary into `stab-model`.
4. Completed: extract `stab-analysis` over model and algebra only, including error matching and matched-error provenance values. Keep it free of records, execution, CLI, and ops.
5. In progress: extract `stab-engine` over model, records, algebra, and analysis; move every A4/A5 compiler, plan, session, and compatibility adapter without changing public behavior.
6. Create dependency-free `stab-kernels-simd`, restore a minimal four-word portable-SIMD implementation against the current scalar references, and make `portable-simd` an additive facade and engine feature with scalar default behavior.
7. Curate `stab-core` root, `advanced`, and `experimental` APIs, and add exact `=0.2.0` path versions to every publishable edge.
8. Add Stable and Nightly consumer fixtures, feature-unification tests, dependency rejection fixtures, rustdoc tier checks, and scalar-versus-SIMD equivalence.
9. Rerun every benchmark family whose call path moved, then run milestone-audit and full-code-review; fix all findings before A6 closure.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, and pure analysis; Stable default builds cannot parse or compile Nightly-only code.
- Only `stab-kernels-simd` may contain `#![feature(portable_simd)]` or direct `std::simd`.
- `stab-kernels-simd` has no Stab dependency and exposes only raw word slices, mutable word slices, and fixed `[u64; 4]` kernels.
- Scalar behavior is the absence of the additive `portable-simd` feature. There are no mutually exclusive scalar and SIMD feature flags.
- CLI, oracle, and benchmark crates enable the facade's `portable-simd` feature explicitly instead of relying on workspace feature unification.
- Product crates never depend on ops or test support at runtime. Stable dev dependencies cannot reach engine, facade, CLI, or ops.
- Every publishable path dependency includes exact version `=0.2.0`; ops and external conformance fixtures remain unpublished.
- Existing `.stim`, `.dem`, result-format, CLI, seeded Stab, statistical Stim, resource, cancellation, poisoning, and sink-lifecycle contracts remain unchanged.
- Move implementation without duplicating it. Compatibility adapters delegate through the new owner and leave the root only where the migration inventory requires.
- A second backend is registered only when portable SIMD executes a genuinely distinct implementation and passes semantic and performance evidence.

## Done Criteria

- The target dependency graph is physical and architecture checks reject every forbidden edge.
- Stable component consumers compile and test on Rust 1.97.1 without `std::simd`; the Nightly facade retains portable-SIMD execution.
- Every product crate documents purpose, dependencies, invariants, resource behavior, extension points, conformance tests, benchmarks, and synchronized files.
- Public API and qualification inventories regenerate exactly, with no qualification-only product item.
- Moved bit, algebra, parser, records, sampler, converter, DEM, and analysis benchmark phases have no unexplained regression.
- Milestone-audit, full-code-review, full verification, and pre-commit have no open A6 finding.

## Required Checks

Use targeted crate and fixture tests after each extraction. Before every focused commit, run formatting, warnings-denied checks for touched crates, targeted tests, architecture enforcement, and staged pre-commit. Before A6 closure, run Stable and Nightly matrices, default and portable-SIMD feature checks, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture and API checks, implemented and result-format oracles, qualification check/regeneration, generated status, benchmark smoke, and source-current phase evidence for every moved call path.
