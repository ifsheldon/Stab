# Migrating To Stab 0.2

Stab 0.2 is one coordinated breaking Rust API release. CLI behavior and the pinned Stim v1.16.0 compatibility target are unchanged, but implementation ownership and lower-level facade paths are now explicit.

## Why The API Changed

The architecture review correctly identified that Stab was agent-friendly as a repository but still presented one broad implementation crate as the toolkit. Stab 0.2 adopts the proposed compiler-style structure: closed Stim-compatible models, pure analysis, immutable execution plans, mutable sessions, typed batch sinks, independent record codecs, and isolated kernels.

We accepted the review's recommendation to expose a small number of strong components instead of one crate per source module. We also kept the Stim gate dialect closed. Custom passes, decoders, and backend seams are not generalized in anticipation of future users; each will be added only after an external implementation proves its contract.

## Product Crates

| Crate | Primary use |
| --- | --- |
| `stab-model` | Stable circuit and detector-error-model values, parsing, printing, validation, identities, and model diagnostics |
| `stab-bits` | Stable packed storage and scalar bit operations |
| `stab-records` | Stable typed batches, layouts, strict Stim result codecs, and sinks |
| `stab-algebra` | Stable Pauli, Clifford, tableau, and flow mathematics |
| `stab-analysis` | Stable pure transforms, generation, circuit-to-DEM analysis, search, SAT, and error matching |
| `stab-engine` | Compilers, immutable plans, mutable sessions, sampling, conversion, and typed batch delivery |
| `stab-decoder` | Stable truth-hidden decoder inputs, caller-owned observable predictions, preflight, cancellation, progress, and statically dispatched session interoperability |
| `stab-kernels-simd` | Dependency-free Nightly portable-SIMD leaf kernels; not a simulation backend |
| `stab-core` | Curated Nightly facade and compatibility conveniences |
| `stab-cli` | The single `stab` command-line binary |

Use a component crate directly when only one layer is needed. Use `stab-core` for an ergonomic integrated toolkit.

## Facade Tiers

| Tier | Contract |
| --- | --- |
| `stab_core::...` | Common models, algebra values, compilers, plans, sessions, batches, diagnostics, policies, and capability discovery |
| `stab_core::analysis::...` | Supported pure-analysis facade functions |
| `stab_core::execution::...` | Supported simulator-backed facade functions and plan/session types |
| `stab_core::advanced::storage::...` | Explicit packed storage |
| `stab_core::advanced::records::...` | Explicit layouts, concrete codecs, materialized helpers, and bounded visitors |
| `stab_core::advanced::backend::...` | Explicit backend selection and descriptors |
| `stab_core::advanced::traversal::...` | Flattened and folded traversal primitives |
| `stab_core::advanced::algebra::...` | Algebra iterators and admitted unchecked constructors |
| `stab_core::advanced::compat::...` | Pre-0.2 materialized and callback-oriented adapters |
| `stab_core::experimental::...` | Implemented extension contracts that may change before 1.0 |

`experimental` contains the circuit-pass contract and built-in without-noise pass proven by a separate Stable crate. Pass implementations must conservatively project folded output resources before lowering; the common executor admits that projection, reports a typed input/projection/output rejection stage, validates the actual closed-model result, and rejects underestimation. Projected payload bytes exclude allocator metadata and spare collection capacity. The canonical pass owner is `stab-analysis`; the facade tier is pre-stable convenience. Decoder interoperability remains available from `stab-decoder` or the facade root. No placeholder backend, GPU, dynamic plugin, or runtime gate-registry traits are published.

The supported `analysis` and `execution` namespaces remain because they describe semantic operations, not physical source ownership. The retired `bits`, `stabilizers`, `result_formats`, and `result_streaming` namespaces mirrored implementation owners and made the facade expand whenever a component changed.

## Common Path Changes

| Pre-0.2 path | Stab 0.2 path |
| --- | --- |
| `stab_core::BitVec` | `stab_core::advanced::storage::BitVec` or `stab_bits::BitVec` |
| `stab_core::bits::BitMatrix` | `stab_core::advanced::storage::BitMatrix` or `stab_bits::BitMatrix` |
| `stab_core::PauliStringIterator` | `stab_core::advanced::algebra::PauliStringIterator` or `stab_algebra::PauliStringIterator` |
| `stab_core::stabilizers::CliffordString` | `stab_core::CliffordString` or `stab_algebra::CliffordString` |
| `stab_core::result_formats::DetsLayout` | `stab_core::advanced::records::DetsLayout` or `stab_records::DetsLayout` |
| `stab_core::result_streaming::for_each_record` | `stab_core::advanced::records::for_each_record` or `stab_records::try_for_each_record` |
| `stab_core::BackendPreference` | `stab_core::advanced::backend::BackendPreference` or `stab_engine::BackendPreference` |
| No circuit-pass facade | `stab_core::experimental::{CircuitPass, run_circuit_pass, CircuitPassContext, CircuitPassLimits, CircuitPassResources, CircuitPassStage}` or canonical `stab_analysis` paths |
| `stab_core::CompiledSampler` | `stab_core::advanced::compat::CompiledSampler`; new code should use `SamplingCompiler`, `SamplingPlan`, and `SamplingSession` |
| `stab_core::CompiledDetectionConverter` | `stab_core::advanced::compat::CompiledDetectionConverter`; new code should use the measurement-to-detection compiler, plan, session, and sink adapter |
| `stab_core::CompiledDemSampler` | `stab_core::advanced::compat::CompiledDemSampler`; new code should use `DemSamplingCompiler`, `DemSamplingPlan`, and a sampling or replay session |

Common model, algebra-value, plan, session, batch, diagnostic, and policy names remain available from the facade root.

Execution compilers, plans, sessions, progress values, summaries, and run errors exposed through `stab_core::execution` are direct reexports of their `stab_engine` owners. Values can move between those paths without wrapper conversion. Code that previously called a facade-only `into_circuit_error` method on a detection or DEM execution error should use `CircuitError::from(error)` when it needs the aggregate facade diagnostic.

## Feature Selection

Scalar behavior is the default for every product crate. Portable SIMD remains additive and opt-in:

```toml
[dependencies]
stab-core = { version = "=0.2.0", features = ["portable-simd"] }
```

The A6 source-current diagnostic found dense XOR effectively neutral and non-identity Clifford composition approximately 1.35 times slower with portable SIMD on the tested AArch64 host. Keeping scalar as the default is an evidence-based selection, not a retreat from the isolated kernel boundary.

## Dependency Versions

Every publishable Stab product package is versioned `0.2.0`, and every publishable path dependency requires exactly `=0.2.0`. This prevents a coordinated release from silently resolving a sibling crate from an incompatible pre-1.0 minor version.

## Post-Review Remediation Changes

The August 2026 remediation ([plans/post-review-remediation-plan.md](plans/post-review-remediation-plan.md), WS1 and WS3) changed the following 0.2-line APIs and semantics before any 0.2 crate publication.

- `CompiledSampler::count_determined_measurements` and `CompiledSampler::reference_sample` now return `CircuitResult`; the duplicate `try_count_determined_measurements` and `try_reference_sample` methods are removed. Append `?` or `.expect(...)` at call sites.
- The panicking `SamplingPlan::count_determined_measurements` and `SamplingPlan::reference_sample` compatibility methods are removed; use `try_count_determined_measurements` and `try_reference_sample`. A parseable hostile circuit can no longer panic a public entry point.
- The free `count_determined_measurements` now returns `Result<u64, CountDeterminedMeasurementsError>`, a two-variant enum over compile and execution errors that converts into `CircuitError`.
- `count_determined_measurements` now matches pinned Stim v1.16.0 semantics: measurement flip arguments are ignored (determinism is a state property), and circuits containing `MPAD` or heralded noise records are rejected with a typed error, mirroring Stim's `count_determined_measurements` rejection of unhandled measurement types.
- `Circuit::count_qubits` now excludes `MPAD` pad values, matching Stim v1.16.0 (`circuit_instruction.cc`); pad values reserve measurement records, not qubits. The previously distinct internal simulated-qubit count is consolidated onto this one owner.
- Reference samples are strictly noiseless: `p == 1` measurement flips invert sampled shots but never the reference bit, matching Stim's dropped-flip reference contract.
- `E`/`ELSE_CORRELATED_ERROR` now accept combiner and inverted Pauli targets as ignored decoration, matching pinned Stim's frame simulator, and every analysis and sampling consumer skips combiners while dropping inversion bits; previously such circuits were rejected at validation.
- Circuit printing now mirrors Stim's `write_targets` exactly: dangling and doubled combiners reprint as stored instead of being dropped or collapsed, and a leading combiner attaches to the preceding header just as pinned Stim prints it.
- `Probability::stim_text()` (with the `ProbabilityStimText` display wrapper) is new API formatting probabilities exactly as pinned Stim prints doubles in generated-circuit headers; `stab gen` headers now use it, so probabilities needing scientific notation or more than six significant digits render byte-identically to Stim.
- Broken-pipe-rooted CLI failures now exit with status 141 and empty stderr, matching pinned Stim's silent `SIGPIPE` death (decision D2); every other output I/O failure keeps its diagnostic and exit 1.
- Legacy mode flags (`--sample`, `--detect`, `--m2d`, `--analyze_errors`, `--convert`, `--gen=`) are accepted anywhere before a `--` separator like pinned Stim, with an adjacent shot count following the flag and an explicit `--shots` elsewhere retained; several mode flags still reject.
- The Stab-specific `sample_dem` observable-routing flags `--append_observables` and `--prepend_observables` are hidden from `--help` (decision D4) but stay functional as compatibility conveniences.
- `FlexPauliString::from_str` rejects doubled sign prefixes (`+-X`, `--X`, `-+X`, `i-X`, `-i+X`) with typed invalid-character errors mirroring pinned Stim instead of silently corrupting the sign.

The detailed decision ledger remains [architecture/0.2-api-migration-inventory.md](architecture/0.2-api-migration-inventory.md). The frozen pre-0.2 inventory remains historical and is not rewritten.
