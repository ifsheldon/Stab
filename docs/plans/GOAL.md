# Goal: Extract The Product Components

## Objective

Finish milestone A6 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): extract the portable-SIMD boundary, curate the facade, and close the consumer-fixture and evidence matrix while preserving the complete A0–A5 behavior and evidence contracts.

## Current State

- A0 through A5 are complete. A5 closes at clean source revision `b8e3f459d2a8817aa98ca0d71072a9529fa9fe9c`.
- The physical product crates are currently `stab-algebra`, `stab-analysis`, `stab-bits`, `stab-engine`, `stab-kernels-simd`, `stab-model`, `stab-records`, `stab-core`, and `stab-cli`.
- `stab-algebra`, `stab-analysis`, `stab-bits`, `stab-model`, `stab-records`, and the current scalar `stab-engine` build on Stable Rust 1.97.1. `stab-model` physically owns the complete circuit and DEM compatibility models, `stab-analysis` physically owns every implemented pure analysis surface, and `stab-engine` physically owns compilation-request fingerprints, execution-side biased randomization, circuit sampling, measurement-to-detection conversion, circuit detection sampling, DEM compilation and execution, reference-sample trees, and sampled-flow execution. `stab-core` preserves compatibility reexports plus materialized and byte-oriented adapters.
- The A6 SIMD audit rejects treating build-time bit or Clifford acceleration as a sampling backend. The first raw-kernel slice is limited to dense XOR and non-identity Clifford composition; explicit `PortableSimd` sampling remains unavailable until a later packed-frame plan exists.
- `ops-contracts` is removed. Qualification policy is oracle-owned, and analyzer benchmarks derive compact-work witnesses from public DEM output instead of hidden product counters.
- Logical ownership, typed diagnostics, resource policies, fingerprints, capabilities, plans, sessions, batches, and sinks are already tested inside the current compilation boundary.
- A6 source work and source-current diagnostic evidence are complete at clean revision `81489b10b561585f898cec46a1faa11380738bdd`; milestone-audit and full-code-review remain before closure.

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
5. Completed: extract circuit sampling, measurement-to-detection conversion, circuit detection sampling, DEM sampling, reference-sample trees, and sampled-flow execution into `stab-engine` without changing facade or CLI behavior.
6. Completed at `a465009c`: create dependency-free `stab-kernels-simd`, restore four-word XOR and Clifford composition against the current scalar references, and keep engine backend registration scalar-only.
7. Completed in source: curate `stab-core` root, `advanced`, and intentionally empty `experimental` tiers; move explicit storage, records, backends, traversal, algebra iterators, and pre-0.2 adapters out of the common root; coordinate every publishable package and path edge at exact `=0.2.0`.
8. Completed at clean revision `81489b10`: the explicit feature map, Stable, scalar-facade, portable-facade, mixed consumers, feature-unification checks, dependency rejection checks, schema-version-7 feature-aware worker receipts, curated facade export inventory, and fixed scalar-versus-SIMD diagnostic harness are implemented. The nine-pair AArch64 diagnostic keeps portable SIMD opt-in because dense XOR is neutral while non-identity Clifford composition is approximately `1.35x` slower. Current CLI, oracle, benchmark, and qualification builds select scalar explicitly.
9. Source-current M5 and M6 correctness, scalar-versus-SIMD, and pinned-Stim timing reports pass their exact semantic and numeric checks. The reports are diagnostic because unrelated sustained host load caused swap and thermal policy violations; formal promotable parity remains an A9 controlled-host requirement.
10. Run milestone-audit and full-code-review, fix every finding, complete the required verification matrix, and only then close A6.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and the scalar engine; Stable default builds cannot parse or compile Nightly-only code.
- Only `stab-kernels-simd` may contain `#![feature(portable_simd)]` or direct `std::simd`.
- `stab-kernels-simd` has no Stab dependency and exposes only raw word slices, mutable word slices, and fixed `[u64; 4]` kernels.
- Scalar behavior is the absence of the additive `portable-simd` feature. There are no mutually exclusive scalar and SIMD feature flags.
- CLI, oracle, and benchmark crates declare their intended scalar or portable build explicitly instead of relying on workspace feature unification.
- Product crates never depend on ops or test support at runtime. Stable development edges may reach Stable product components such as `stab-engine` and test-support fixtures, but they cannot make a Stable default build reach the Nightly facade, CLI, portable-SIMD kernel, or ops.
- Every publishable path dependency includes exact version `=0.2.0`; ops and external conformance fixtures remain unpublished.
- Existing `.stim`, `.dem`, result-format, CLI, seeded Stab, statistical Stim, resource, cancellation, poisoning, and sink-lifecycle contracts remain unchanged.
- Move implementation without duplicating it. Compatibility adapters delegate through the new owner and leave the root only where the migration inventory requires.
- A second backend is registered only when portable SIMD executes a genuinely distinct implementation and passes semantic and performance evidence.

## Done Criteria

- The target dependency graph is physical and architecture checks reject every forbidden edge.
- Stable component consumers compile and test on Rust 1.97.1 without portable-SIMD code; Nightly consumers can opt into measured leaf kernels without changing sampling capability claims.
- Every product crate documents purpose, dependencies, invariants, resource behavior, extension points, conformance tests, benchmarks, and synchronized files.
- Public API and qualification inventories regenerate exactly, with no qualification-only product item.
- Moved bit, algebra, parser, records, sampler, converter, DEM, and analysis benchmark phases have no unexplained regression.
- Milestone-audit, full-code-review, full verification, and pre-commit have no open A6 finding.

## Required Checks

Use targeted crate and fixture tests after each extraction. Before every focused commit, run formatting, warnings-denied checks for touched crates, targeted tests, architecture enforcement, and staged pre-commit. Before A6 closure, run Stable and Nightly matrices, default and portable-SIMD feature checks, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture and API checks, implemented and result-format oracles, qualification check/regeneration, generated status, benchmark smoke, and source-current phase evidence for every moved call path.
