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
| `stab_core::advanced::traversal::...` | Flattened and folded traversal primitives |
| `stab_core::advanced::algebra::...` | Algebra iterators and admitted unchecked constructors |
| `stab_core::advanced::compat::...` | Removed. Use the owning compiler, plan, session, and sink APIs. |
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
| No circuit-pass facade | `stab_core::experimental::{CircuitPass, run_circuit_pass, CircuitPassContext, CircuitPassLimits, CircuitPassResources, CircuitPassStage}` or canonical `stab_analysis` paths |
| `stab_core::CompiledSampler` | Removed. Compile with `SamplingCompiler`, create a `SamplingSession`, and deliver output through a `MeasurementSink`. Use `MeasurementCodecSink` when encoded bytes are the intended result. |
| `stab_core::CompiledDetectionConverter` | Removed. Compile with `MeasurementToDetectionCompiler`, create a `MeasurementToDetectionSession`, and route records through `MeasurementToDetectionSinkAdapter` into a `DetectionSink`. |
| `stab_core::CompiledDemSampler` | Removed. Compile with `DemSamplingCompiler`, create a sampling or replay session, and deliver typed batches through `DemSampleSink`. |

Common model, algebra-value, plan, session, batch, diagnostic, and policy names remain available from the facade root.

Sampling has one executable scalar backend. `SamplingPlan::backend` and `PlanFingerprint::backend` report that actual implementation identity; there is no caller-selectable backend preference, registry, or unavailable-backend placeholder. The additive `portable-simd` feature accelerates leaf kernels and does not create a second sampling plan.

Execution compilers, plans, sessions, progress values, summaries, and run errors exposed through `stab_core::execution` are direct reexports of their `stab_engine` owners. Values can move between those paths without wrapper conversion. Code that previously called a facade-only `into_circuit_error` method on a detection or DEM execution error should use `CircuitError::from(error)` when it needs the aggregate facade diagnostic.

## Removed Pre-0.2 Residue

The following public items had no product, test, benchmark, example, or documented external consumer at the frozen pre-0.2 boundary and are removed without compatibility adapters.

- `stab_bits::BitError::MatrixShapeMismatch` is removed. No bit operation constructed it; matrix operations continue to report their actual `LengthMismatch`, `NotSquareMatrix`, row, size, or allocation errors.
- `stab_core::CircuitError::{ParseLine, UnterminatedRepeatBlock, UnexpectedRepeatTerminator}` are removed. Circuit and DEM parsing now reports the typed `CircuitError::Parse(ParseError)` contract with stable codes, spans, and context.
- `stab_analysis::advanced::{ReverseFlowTransition, reverse_flow_transition, check_unsigned_flows_with_sparse_tracker, AnalyzerProbeBudget, ShiftedRecurrence, ShiftedRecurrenceSearch, SparseReverseFrameTracker, search_shifted_recurrence}` are removed from the public namespace. They were implementation details of supported flow checking, circuit-to-DEM conversion, and error-analysis operations; use those high-level APIs instead.

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

The August 2026 remediation ([plans/post-review-remediation-plan.md](plans/post-review-remediation-plan.md), WS1, WS3, and WS5) changed the following 0.2-line APIs and semantics before any 0.2 crate publication.

- `CompiledSampler`, its materialized record methods, byte encoders, and callback visitors are removed. Use `SamplingPlan::try_count_determined_measurements`, `SamplingPlan::try_reference_sample`, or the sweep-aware `circuit_reference_sample` operation for reference work, and use a typed sink for sampling output.
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
- `Circuit` now implements `Drop`, `Clone`, and `PartialEq` iteratively (mirroring `DetectorErrorModel`), so deeply nested API-built circuits no longer abort the process; its `Debug` output elides nested bodies (`Circuit { top_level_items: N, .. }`), and moving `items` out of a `Circuit` by value is no longer possible because the type implements `Drop`.
- `CompiledDetectionConverter`, its facade forwards, whole-output DTOs, callback visitors, and byte writers are removed. Use `MeasurementToDetectionCompiler`, its immutable plan and mutable session, and a typed `DetectionSink`; scratch allocation and execution failures surface through the owning engine error types.
- `CompiledDemSampler`, `DetectionEventRecord`, DEM whole-output materializers, and callback visitors are removed. Use `DemSamplingCompiler`, `DemSamplingPlan`, `DemSamplingSession` or `DemReplaySession`, and `DemSampleSink`. `DemSamplerLimits::max_active_batch_bytes` now names the reusable session storage it actually bounds; the obsolete whole-output unit budget is removed.
- `stab_records::RecordStreamReader::next_b8_packed_record` and its `stab_core::advanced::records` reexport expose one borrowed, validated B8 frame without dense expansion. The frame preserves input padding bits; consumers that emit canonical B8 must clear bits beyond the declared record width before writing.

The detailed decision ledger remains [architecture/0.2-api-migration-inventory.md](architecture/0.2-api-migration-inventory.md). The frozen pre-0.2 inventory remains historical and is not rewritten.
