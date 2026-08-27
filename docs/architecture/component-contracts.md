# Component Contract Template

Each product component must document the following fields.

## Template

### Purpose

State the single responsibility of the component.

### Public Inputs And Outputs

List typed values crossing the component boundary.

### Owned Invariants

List invariants that this component validates and preserves.

### Dependencies

List permitted product dependencies and explain each one.

### Forbidden Dependencies

List components that must not be imported.

### Resource Behavior

Describe admission policy, materialization, scratch reuse, cancellation, and worst-case growth.

### Extension Points

List supported composition boundaries.

### Conformance Tests

Name semantic, compatibility, negative, and resource test families.

### Benchmarks

Name phase-specific workload families and their comparator class.

### Files Changed Together

List source descriptors, generated files, docs, tests, and benchmark metadata that must remain synchronized.

## Initial Ownership

### Model

- Purpose: own exact Stim circuit and DEM values plus syntax.
- Inputs and outputs: bytes or text, typed models, gate and target values, IDs, parse or validation errors, cheap resource estimates, and bounded borrowed DEM error-mechanism views with absolute typed targets.
- Invariants: closed Stim v1.16.0 dialect, canonical text and byte printing, opaque comment payloads, exact unescaped tag bytes, structural validity, typed indices and probabilities, exact byte spans, bounded diagnostic text, honest estimate classifications, and one model-owned interpretation of DEM repeats, shifts, separators, and absolute error targets.
- Dependencies: foundational standard-library, compact-storage, hashing, and error-reporting support; no other Stab product crate.
- Forbidden: engine, records, CLI, ops.
- Resource behavior: `ParseLimits` owns caller-selectable source-line admission and a caller-tightenable 256-level parsed-model repeat envelope. Programmatic models may exceed that parser envelope only for APIs with an established deeper contract; those consumers must be iterative or reject before recursive work. `DemErrorMechanismTraversalLimits` independently admits represented mechanisms and represented instruction visits before or during callback traversal, skips error-free repeat subtrees, and never allocates a target vector per mechanism. `DemFlattenLimits` belongs to the analysis adapter rather than the model.
- Extension points: immutable typed circuit passes consume and return models; bounded error-mechanism visitors let decoders consume DEM semantics without importing the advanced folded execution cursor.
- Conformance tests: pinned-Stim circuit and DEM parsing and printing, exact byte spans and diagnostics, gate and target validation, typed identifier and probability bounds, repeat-depth admission, folded traversal, shifted and nested error mechanisms, mechanism and instruction-work limits, early visitor stop, canonical fingerprints, and external Stable consumers.
- Benchmarks: circuit and DEM parsing and printing, gate lookup, folded traversal, and model fingerprinting as separately timed phases with pinned-Stim comparators where the semantic work is faithful.
- Files changed together: model source and tests, facade value reexports, parser and dialect fixtures, parity ownership, affected runtime contracts and profiler notes, and architecture documentation.

### Bits

- Purpose: own Stable packed bit storage, checked views, scalar kernels, sparse symmetric-difference storage, and bit-matrix layouts.
- Inputs and outputs: typed bit vectors, slices, blocks, matrices, sparse index sets, and `BitError`.
- Invariants: logical widths are explicit, unused tail bits are masked at owned boundaries, checked operations reject width and range mismatches before mutation, and optional acceleration cannot change scalar semantics.
- Dependencies: standard-library support plus the optional dependency-free raw SIMD kernel crate behind `portable-simd`.
- Forbidden: model, records, algebra, analysis, engine, facade, CLI, and ops.
- Resource behavior: owned storage is proportional to declared logical width or matrix shape; in-place kernels allocate nothing after construction; sparse operations retain capacity proportional to their canonical sorted support.
- Extension points: checked packed storage and allocation-free in-place operations. New raw acceleration remains private to the selected implementation boundary until a measured caller proves it.
- Conformance tests: scalar references, tails, dirty padding, empty and block boundaries, unequal-width rejection without mutation, sparse invariants, transpose semantics, allocation counts, and default-versus-portable feature runs.
- Benchmarks: dense XOR, not-zero, popcount, sparse XOR, and transpose workloads with pinned Stim comparators where faithful; scalar-versus-SIMD diagnostics apply only to kernels whose executed path changes.
- Files changed together: bit source and tests, optional feature maps, raw kernel sources, architecture checks, qualification ownership, runtime contracts, profiler notes, and Stable/Nightly matrix documentation.

### Records

- Purpose: own typed result layouts, packed batches, strict codecs, sources, and sinks.
- Inputs and outputs: measurement, detection, observable, sampled-error, and prediction batches, including immutable and mutable checked prediction prefixes.
- Invariants: explicit width, namespace, layout, tail-bit, record-boundary, PTB64 grouping, correction-width, and mutable-prefix contracts.
- Dependencies: packed bit storage.
- Forbidden: circuit execution, filesystem paths, CLI, ops.
- Resource behavior: dense and packed codec scratch is bounded by declared record width and the active batch, never total stream length or duplicate-token count. Raw sparse and typed-token visitors may retain one encoded record because duplicate order is their returned value. An explicitly in-memory codec sink may retain its caller-requested encoded output bytes; that materialized result is reported separately from bounded working scratch. Mutable prediction views borrow existing storage, allocate nothing, and cannot address reusable suffix records beyond their admitted shot prefix.
- Extension points: typed record sinks, bounded record visitors, truth-hiding decoder inputs, and correction-typed mutable prediction prefixes.
- Conformance tests: the pinned-Stim result-format corpus across materialized and streaming readers, exact width and namespace bounds, malformed grammar, PTB64 groups, duplicate-token semantics, visitor cancellation, mutable prediction-prefix isolation, bounded allocation, sink lifecycle, and external Stable consumers.
- Benchmarks: phase-specific `01`, `b8`, `r8`, HITS, DETS, and PTB64 decode and encode workloads, plus process-symmetric conversion rows when the CLI boundary is the public operation under study.
- Files changed together: records source and tests, shared compatibility corpus, facade value reexports, CLI conversion and replay paths, oracle fixtures, parity ownership, benchmark runtime contracts and profiler notes, and format documentation.

### Algebra

- Purpose: own Pauli, Clifford, Tableau, and Flow mathematics.
- Inputs and outputs: algebraic values and typed algebra errors.
- Invariants: exact phase, sign, commutation, composition, and resource admission.
- Dependencies: packed bit storage, plus an optional raw SIMD kernel edge selected only on Nightly.
- Forbidden: Stim parsing, CLI, ops.
- Resource behavior: scalar stable defaults with bounded construction and optional Nightly acceleration above storage kernels.
- Extension points: pure functions and owned values.
- Conformance tests: exact Pauli phase and commutation laws, all valid Clifford products, tableau composition and inversion, flow semantics, width and resource rejection, scalar-versus-SIMD equivalence, allocation bounds, and external Stable consumers.
- Benchmarks: Pauli, Clifford, tableau, and flow phases with exact semantic witnesses, including scalar-versus-SIMD diagnostics only for kernels whose executed path changes and pinned-Stim comparators where faithful.
- Files changed together: algebra source and tests, bits and optional kernel feature maps, facade value reexports, analysis and engine callers, parity ownership, affected runtime contracts and profiler notes, and Stable/Nightly documentation.

### SIMD Kernels

- Purpose: own the complete Nightly and direct portable-SIMD boundary.
- Inputs and outputs: fixed four-word blocks and fixed arrays of raw Clifford bit planes.
- Invariants: no Stab domain dependency, exact scalar-equivalent XOR and Clifford composition, no tail policy, no allocation, and no backend registration.
- Dependencies: standard library only.
- Forbidden: every Stab product crate, model value, record layout, execution plan, CLI type, and ops descriptor.
- Resource behavior: constant stack scratch over fixed arrays; callers own block iteration, scalar tails, logical tail masking, width validation, and allocation policy.
- Extension points: new kernels are added only after a production caller, independent scalar reference, focused semantic tests, and source-current performance evidence exist.
- Conformance tests: raw fixed-block differential formulas, public boundary and tail tests in the owning Stable crate, all 24-by-24 valid Clifford products, exact metadata counts, and warmed zero-allocation mutation.
- Benchmarks: scalar-versus-SIMD dense XOR and non-identity Clifford right multiplication, followed by the existing Stim-relative parity rows for the selected build.
- Files changed together: kernel source and manifest, owning Stable feature maps, architecture checks, qualification ownership, affected profiler notes, feature-aware worker identity, and architecture documentation.

### Engine

- Purpose: compile exact models into plans and execute reusable sessions.
- Inputs and outputs: models, compiler options, plans, sessions, typed batches, execution summaries.
- Invariants: private executable IR, one actual implementation identity per plan, session-owned mutable state, bounded execution batches.
- Dependencies: model, records, algebra values, and pure analysis lowering.
- Forbidden: textual codecs, filesystem paths, CLI, ops.
- Resource behavior: `DetectionConversionLimits` and `DemSamplerLimits` own the caller-selectable conversion and DEM execution budgets. Circuit sampling, measurement-to-detection conversion, circuit detection sampling, and DEM sampling each have operation-specific compiler, immutable-plan, owned mutable-session, and typed-sink boundaries. Session construction uses fallible reservation and conservatively rejects more than 256 MiB of reusable frame, reference, record, error, and bit-plane storage before allocation; fused detection admits the aggregate sampling-plus-conversion estimate, not each component independently. DEM replay work and active retained bytes remain distinct limits. Sampling and replay sessions reuse at most one 64-shot batch, retain no caller-owned model or sink borrow, and replay reset reuses only a completed non-poisoned lifecycle. Conversion may accept smaller caller batches or one record at a time, and cancellation is checked only at documented batch boundaries. One expensive shot has no wall-clock cancellation deadline.
- Extension points: measurement, detection, and DEM-sample sinks.
- Conformance tests: seeded reproducibility, pinned-Stim statistical semantics, compilation and plan fingerprints, fixed implementation identity, session reuse, cancellation and poisoning, resource admission, sink lifecycle, reference samples, sampled flows, and composed sampling-to-detection behavior.
- Benchmarks: compilation, execution, conversion, and delivery phases for circuit sampling, detection conversion, fused detection, and DEM sampling with exact work and output witnesses; aggregate facade or CLI timing cannot substitute for an owning engine phase.
- Files changed together: engine source and tests, model and analysis lowering contracts, records sink interfaces, facade namespace exposure, capability and plan schemas, parity ownership, runtime contracts and profiler notes, and resource documentation.

### Analysis

- Purpose: own pure transforms, circuit-to-DEM analysis, search, generation, and error matching.
- Inputs and outputs: immutable models, transformed models, typed pass options, contexts, reports and diagnostics, analysis errors.
- Invariants: no hidden execution session or filesystem state, explicit folded or materialized resource behavior, framework admission before external pass dispatch, conservative logical-payload projection before proportional lowering allocation, typed input/projection/output rejection stages, and closed-dialect actual-output validation before a pass result is returned.
- Dependencies: model and algebra.
- Forbidden: CLI, ops, and engine.
- Resource behavior: `CircuitPassLimits` independently admits represented items, targets, arguments, projected logical payload bytes, and repeat nesting without expanding repeat counts. Payload bytes cover logical item, target, argument, and exact plus lossy opaque-tag data but exclude allocator metadata and spare collection capacity, so they bound proportional model work rather than exact resident memory. Input is admitted before dispatch, each implementation supplies a checked conservative output projection before proportional output allocation, and the common executor validates the actual output against both the caller's limits and that projection. `CircuitFlattenLimits`, `DemFlattenLimits`, `LogicalErrorSearchLimits`, and `SatMaterializationLimits` own their independent expansion, retained-state, and output budgets. Other partial analysis algorithms retain documented fixed safety contracts instead of sharing a generic policy.
- Extension points: statically composed typed circuit passes over the closed `Circuit` model. Passes own typed options, reports, diagnostics, and output projection; the common executor owns input admission, projected-output admission, actual-output validation, and projection-underestimate rejection.
- Conformance tests: pinned-Stim transforms and circuit-to-DEM behavior, exact generated circuits and decompositions, tableau and flow analysis, logical-error and SAT witnesses, error matching and provenance, folded-repeat handling, deterministic resource rejection, and external Stable consumers.
- Benchmarks: circuit-to-DEM lowering, transforms, generation, flow solving, logical-error search, SAT materialization, and error matching as separate semantic phases with exact work witnesses and pinned-Stim comparators where faithful.
- Files changed together: analysis source and tests, model and algebra contracts, facade namespace exposure, generator and oracle fixtures, parity ownership, affected runtime contracts and profiler notes, and operation-specific resource documentation.

### Decoder

- Purpose: own Stable interoperability between decoder implementations, truth-hidden detector batches, caller-owned observable predictions, and reusable statically dispatched sessions.
- Inputs and outputs: borrowed DEM model views and fingerprints, detector-only batch views, decoder layouts, mutable prediction prefixes, cancellation tokens, exact progress summaries, and typed preflight, implementation, or contract failures.
- Invariants: observable truth never crosses the decoder input boundary; detector width, correction width, and prediction capacity are checked before implementation dispatch or mutation; completed predictions form one prefix; session layout remains fixed during a call; and the hot path uses static dispatch.
- Dependencies: model for exact DEM identity and records for typed detector and observable-prediction storage.
- Forbidden: engine, analysis, facade, CLI, ops, private executable IR, textual codecs, filesystems, dynamic Rust plugins, and Nightly-only features.
- Resource behavior: the common boundary borrows input and output storage, allocates only when constructing an implementation-specific diagnostic, checks pre-cancellation before dispatch, and leaves compilation and retained-state admission to each decoder implementation. The bounded reference implementation admits at most 20 detectors, one observable, 256 represented mechanisms, and 65,536 represented instruction visits; it uses mutually exclusive 32 MiB directed-interval and exact-dyadic probability workspaces, rejects exact tables above that cap or exact work above `2^28` pair-transition limbs, retains one measured allocation of one byte per detector syndrome, and allocates nothing during reused decoding. Cancellation is observed between records and carries no wall-clock deadline for one record.
- Extension points: downstream Cargo crates implement `DecoderSession` and choose their own compiler, retained representation, and algorithms. `decode_batch` is the canonical validation and execution entry point; no universal compiler or runtime registry is implied.
- Conformance tests: dimension rejection before dispatch or mutation, truth-hidden detection input, zero-shot and pre-cancel behavior, committed-prefix cancellation and failure, malformed implementation summaries, fixed layout, external Stable consumers, and a separate bounded exact-ML implementation using public APIs only.
- Benchmarks: implementation-specific compilation, reused decoding, and complete pipeline phases are Stab self-regression workloads unless a faithful external comparator exists; the common trait does not create a synthetic Stim ratio.
- Files changed together: decoder source and tests, records prediction views, model error-mechanism traversal, facade namespace and value reexports, architecture policy, Stable consumers, parity ownership, selected runtime contracts and profiler notes, and architecture documentation.

### Facade

- Purpose: provide one ergonomic `stab-core` dependency without obscuring canonical component ownership.
- Inputs and outputs: common owned model, algebra, decoder, and record values plus direct `analysis`, `decoder`, and `execution` component namespaces.
- Invariants: every root value is inventory-owned, namespace aliases preserve component type identity, and the facade owns no algorithm, duplicate model, universal error, backend registry, codec, or low-level storage API.
- Dependencies: records, algebra, model, analysis, engine, and decoder.
- Forbidden: CLI, bits as a direct facade surface, ops, test-support runtime dependencies, direct portable-SIMD imports, forwarding modules, and compatibility adapters.
- Resource behavior: the facade performs no runtime work and retains no operation output or execution scratch.
- Extension points: none. Circuit passes and decoder sessions are exposed by their owner crates and their direct facade namespaces.
- Conformance tests: exact root inventory, forbidden escape routes, executable scalar and portable external consumers, direct/facade type identity through real workflows, and feature isolation.
- Benchmarks: none. Component and CLI workflows are measured at their actual public boundary.
- Files changed together: `stab-core` exports, `ops/architecture/facade-root-reexports.txt`, architecture policy and tests, external consumers, README, and migration documentation.

### CLI

- Purpose: adapt typed product APIs to Stim-compatible process behavior and Stab-native agent commands.
- Inputs and outputs: Clap arguments, retained input and output file roles, stdin/stdout/stderr, exit status, human or JSON diagnostics, and source-owned capability or plan representations.
- Invariants: implemented Stim flags and file formats preserve their compatibility contracts, path aliases fail before truncation, agent schemas are versioned, and command code does not duplicate model, codec, compiler, backend, or qualification truth.
- Dependencies: model, records, analysis, engine, bits where command-local packed storage is required, plus command-line and operating-system adapter libraries.
- Forbidden: private component internals, direct kernel selection, benchmark policy, correctness ledgers, and mutable global extension registries.
- Resource behavior: command preflight opens and validates all active file roles before output truncation, streaming commands retain bounded per-record or per-batch state, and bounded materialized commands report their explicit input and output caps.
- Extension points: versioned Stab-native inspection, capability, and planning commands over public descriptors. Product extensions enter through component APIs before becoming CLI switches.
- Conformance tests: exact and structural pinned-Stim command cases, malformed input, stderr and exit classes, file alias identities, large streaming requests, and agent-schema fixtures.
- Benchmarks: process-symmetric CLI rows for startup, parsing, conversion, sampling, detection, DEM work, and output routing; in-process rows remain diagnostic unless they faithfully match the public process boundary.
- Files changed together: command definitions and tests, help and agent schemas, capability descriptors, oracle fixtures, benchmark manifest rows, path-role policy, README, and feature checklist.

## Current Source Ownership

This table records source-current physical ownership. `stab-core` appears only where it provides a direct namespace or an identity-preserving item from the finite root inventory.

Nested `tests.rs` and resource-test modules inherit the owner of their parent source family.

| Current source family | Logical owner | Migration note |
| --- | --- | --- |
| `crates/stab-model/src/circuit.rs`, `circuit/**`, `model_bytes.rs`, `model_parse.rs`, `model_tag.rs`, `source_text.rs` | Model | Circuit syntax, values, byte-aware parsing, canonical text and byte printing, iteration, structural counts, opaque tags, and operation-owned parse admission are physically model-owned. The shared byte preparation path preserves source-order failures and opaque Stim metadata without lossy whole-input UTF-8 conversion. Programmatic depth beyond the parser envelope remains consumer-specific rather than being silently accepted by recursive algorithms. A6 removed algorithmic inherent adapters; analysis and execution behavior is reached through named owner functions. |
| `dem.rs`, `dem/api.rs`, `dem/coordinate_scan.rs`, `dem/error_mechanisms.rs`, `dem/parser.rs`, `dem/tag.rs`, `dem/traversal.rs` | Model | The DEM model shares the byte-aware model preparation contract and retains exact opaque tag bytes. Folded traversal is the model-owned advanced boundary shared by DEM queries, analysis, and execution. The stable bounded error-mechanism visitor is a narrower downstream seam that applies repeats and detector shifts, preserves separators and duplicates, yields typed absolute targets without per-mechanism allocation, skips error-free repeat bodies, and admits mechanism count plus represented instruction work. Repeat selections contain model facts and ceilings only; visitors construct consumer-owned expansion failures so logical-search and SAT resource identities do not leak into model types. Consumer-specific search, filtering, parity, and probability policies remain with their owning consumers, while compact transforms live under `crates/stab-analysis/src/dem/**` and use explicit stacks. |
| `crates/stab-model/src/gate/**` | Model | The closed Stim gate registry, aliases, syntax validation, scalar unitary rows, raw flow strings, and raw decomposition text are physically model-owned. `stab-core` reexports the model-owned `Gate` value identity-preservingly from its finite root inventory; algebra-valued projections and decomposition parsing are owned by `stab-analysis`. |
| `crates/stab-analysis/src/circuit.rs`, `circuit_pass.rs`, `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs`, `circuit_generation.rs`, `circuit_generation/**`, `circuit_flow.rs`, `circuit_flow/**`, `circuit_inverse.rs`, `circuit_inverse/**`, `circuit_feedback.rs`, `circuit_detecting_regions.rs`, `circuit_detecting_regions/**`, `circuit_missing_detectors.rs`, `circuit_missing_detectors/**`, `circuit_to_dem.rs`, `circuit_to_dem/**`, `dem.rs`, `dem/**`, `sparse_rev_frame_tracker.rs`, `sparse_rev_frame_tracker/**`, `mbqc_decomposition.rs`, `gate.rs`, `error.rs`, `resources.rs` | Analysis | The Stable analysis crate owns the admitted typed circuit-pass contract, circuit and DEM transforms, generation, algebra-valued gate projections, flow analysis, sparse reverse tracking, circuit-to-DEM lowering, error matching, logical search, and SAT/WCNF materialization. Its root exports and `AnalysisError` are canonical; `stab_core::analysis` is a direct crate alias with no wrapper behavior. Operation-specific policies and failures stay beside their algorithms. |
| `crates/stab-model/src/ids.rs`, `crates/stab-model/src/target.rs` | Model | Typed identifiers, targets, and validated probability primitives are physically owned by the Stable model package. Construction returns `ModelError`; `stab-core` may reexport common values without changing their error type. |
| `crates/stab-model/src/diagnostics.rs`, `dialect.rs`, `parse_limits.rs`, `resource_limit.rs`, `resources.rs`, `validation.rs`, and `error.rs` | Model | Stable byte spans, parser diagnostics, model dialect identity, parser limits, structural validation, and the shared estimate vocabulary are model-owned. Attacker-controlled diagnostic text remains UTF-8-safe and bounded, while `ModelError` aggregates typed parse, parse-resource, and validation failures. Internal constructors remain private or explicitly advanced only where analysis or engine code needs checked construction without exposing model storage. |
| `fingerprint.rs` | Model | Versioned circuit and DEM identities stream dialect-separated structural model encodings into SHA-256 without depending on compatibility-printer precision or allocating model-sized text. An explicit traversal stack is inline through the parser's repeat envelope and spills by depth only for deeper programmatic models. Compilation-request and backend-bearing plan identities remain with engine compilation rather than extending the model fingerprint. |
| `crates/stab-engine/src/fingerprint.rs`, `probability.rs`, `sampling/mod.rs`, `sampling/**`, `sampling_estimate.rs`, `detection/mod.rs`, `detection/**`, `dem_sampling/mod.rs`, `dem_sampling/**`, `reference_sample_tree.rs`, `sampled_flow.rs` | Engine | Backend-neutral request identity, execution-side biased randomization, sampling request estimates, sampling, detection, and DEM capability descriptors, compilation, immutable plans, owned mutable sessions, direct-Z, small-frame, general-frame, deterministic reference-sample, measurement-to-detection conversion, direct detector-frame and fused detection sampling, lowered folded DEM execution, detector-only and sampled-error DEM sampling, owned reusable replay state, bounded reference-sample trees, sampled-flow execution, cancellation, progress, poisoning, and typed measurement, detection, or DEM-sample delivery are physically engine-owned. Incremental conversion and replay use short-lived transactions that bind exactly one sink across writes and finalization; reusable plans and sessions retain no caller borrow. The engine imports model, records, algebra, and analysis but no facade, codec, filesystem, CLI, or ops API. Its crate root is the sole canonical public execution namespace. |
| `crates/stab-decoder/src/**` | Decoder | Stable truth-hidden detector input, model and layout views, caller-owned prediction prefixes, cancellation, preflight, progress, implementation failures, contract validation, and static `DecoderSession` dispatch are physically decoder-owned. Compilation and retained decoder representations remain implementation-specific. |
| `test-support/reference-decoder/src/**` | External decoder proof | The unpublished exact-ML implementation consumes only public Stable model, records, and decoder APIs at runtime. Its fixed workspace, pair-update, and limb-weighted work limits, directed-interval probability program with exact-dyadic posterior certification, allocator-measured one-byte-per-syndrome retained table, and allocation-free reused decode prove the common seam without becoming product or CLI behavior; analysis and engine are test-only experiment dependencies. |
| `test-support/reference-noise-pass/src/**` | External circuit-pass proof | The unpublished deterministic X-noise insertion pass consumes only public Stable analysis and model APIs. It projects inserted item, target, argument, and logical payload growth before allocation, recursively preserves the original closed-dialect structure and opaque metadata, inserts validated `X_ERROR` instructions after eligible represented single-target unitary instructions, returns a typed report, and proves the pass seam without depending on the facade, engine, CLI, ops, Nightly, portable SIMD, or a runtime registry. |
| `crates/stab-core/src/lib.rs` | Facade | Aliases `analysis`, `decoder`, and `execution` directly to their component crates and reexports the source-owned root inventory checked by `ops/architecture/facade-root-reexports.txt`. It contains no public modules, algorithms, universal errors, or low-level tiers. |
| `crates/stab-bits/src/**` | Bits | Stable Rust 1.97.1 packed storage, checked views, scalar kernels, sparse XOR storage, and transpose implementation are physically extracted. The optional `portable-simd` feature reaches only the dependency-free raw kernel crate and replaces complete four-word XOR blocks while retaining scalar tails and logical tail masking locally. |
| `crates/stab-algebra/src/**` | Algebra | Stable Pauli, Clifford, tableau, flow, conversion, error, resource, and scalar quantum-word implementations are physically extracted. The crate depends on `stab-bits` and optionally on the raw kernel crate. Low-level unchecked construction used by admitted analysis and execution algorithms is isolated under `stab_algebra::advanced`; the optional feature replaces complete four-word Clifford composition blocks while retaining scalar tails, public metadata, and resource policy locally. |
| `crates/stab-kernels-simd/src/**` | SIMD kernels | The dependency-free Nightly crate owns the only direct portable-SIMD feature gate and imports. Its first surface is fixed-block XOR plus Clifford right multiplication with before-and-after non-identity masks; it owns no Stab types, logical tails, allocation, or sampling backend policy. |
| `crates/stab-records/src/**` | Records | Stable Rust 1.97.1 strict codecs, typed semantic widths, shot-major and 64-shot bit-plane batches, correction-typed mutable prediction prefixes, typed DETS layouts, bounded visitors, and measurement, detection, and DEM-sample sinks are physically owned here. Shared text and packed decoders own grammar and length diagnostics so materialized and streaming consumers cannot drift. The CLI and facade consume these APIs directly rather than through adapters. |
| `crates/stab-analysis/src/dem/search.rs`, `dem/search/**` | Analysis | These consume the model-owned folded traversal boundary and physically own compact detector indexing, bounded error-mechanism traversal, graphlike and hypergraph search, and operation-specific traversal, graph, hyperedge, and frontier resource policies. |
| `crates/stab-analysis/src/error_matcher.rs`, `error_matcher/**`, `matched_error.rs` | Analysis | These physically own pure error matching, compact filter traversal, resource admission, canonical provenance values, ordering, and diagnostic formatting; simulator-backed sampled-flow checks remain execution-owned. |

`stab-cli/src/agent.rs` is a CLI adapter, not a new product component. It discovers commands from Clap, renders model, record, and engine descriptors and identities directly, reuses retained-handle input admission, and may compose parsing, compilation validation, and request estimates. It must not become an alternate source of gate, codec, compiler, backend, or qualification truth.

Packed storage is available from `stab-bits`, records and codecs from `stab-records`, traversal from `stab-model`, and low-level algebra from `stab-algebra`. The facade root remains a finite mechanically checked convenience inventory; algorithms and lower-level implementation contracts use their component namespace or crate.

New source modules must fit exactly one row or update this table and the architecture decision record in the same change.
