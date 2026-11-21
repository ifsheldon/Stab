# Stab Product Architecture

This directory defines Stab's active product architecture.

The active milestone, test contract, and acceptance criteria live in [the Stim core parity and lean evidence plan](../plans/stim-core-parity-and-lean-evidence-plan.md), and release evidence in [GOAL.md](../plans/GOAL.md).

The source-current public Rust migration guide is [Migrating to Stab 0.2](../MIGRATING-0.2.md). Superseded extraction maps, migration ledgers, and package baselines remain available in Git history.

The stable byte contract for source-model identities is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

The stable byte contract for backend-neutral compiler inputs is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

The stable byte contract for backend-bearing executable identities is defined by [plan fingerprint schema version 1](plan-fingerprint-schema-v1.md).

The current successful machine-output contracts are [capabilities schema version 6](agent-cli-capabilities-schema-v6.md), [plan schema version 5](agent-cli-plan-schema-v5.md), and inspect in [agent CLI schema version 2](agent-cli-schema-v2.md). Superseded schemas remain available in Git history and are not emitted as compatibility views.

The classification of every resource limit, including each removed one, is recorded in [the resource policy inventory](resource-policy-inventory.md).

## Architectural Center

```text
Stim source bytes
    -> parsed model
    -> structural validation
    -> explicit transforms and analysis
    -> private canonical executable IR
    -> backend-specific immutable plan
    -> mutable reusable session
    -> typed packed batches
    -> codecs, decoders, statistics, or other sinks
```

The primary product boundary is:

> typed models -> explicit passes -> opaque plans -> reusable sessions -> typed streams

## Compatibility Boundary

Stim v1.16.0 remains the frozen compatibility target.

`Circuit`, `DetectorErrorModel`, Stim gate syntax, and result formats are closed compatibility models.

Custom research behavior lowers into those models through typed passes.

Stab does not extend the Stim gate table through a runtime registry.

## Product Components

| Component | Owns | Must not own |
| --- | --- | --- |
| Model | Circuit and DEM values, the closed `GateKind` identity and its one gate semantics table, targets, IDs, parsing, printing, structural validation, folded circuit summaries | Simulation, analysis algorithms, CLI, files |
| Foundation | The memory budget and its refusal, cooperative cancellation, resource estimates, and the byte span and severity of a diagnostic: small value types that several components put in their signatures | Models, bits, formats, gates, any description of which operation was refused, any dependency |
| Bits | Packed storage, checked views, scalar kernels | Quantum semantics, formats, execution |
| Records | Measurement and detection layouts, packed batches, codecs, sources, sinks | Circuit execution, filesystem paths |
| Algebra | Pauli, Clifford, Tableau, Flow semantics | Stim parsing, CLI, operational plans |
| SIMD kernels | Slice-level `fearless_simd` word kernels and the only dependency on the SIMD library | Models, codecs, policy, filesystem access, Stab types |
| Engine | Compilation, private IR, plans, sessions, sampling, detection conversion, DEM sampling | Text codecs, paths, CLI, ops |
| Analysis | Pure circuit and DEM transforms, search, generation | CLI, operational plans, mutable execution sessions |
| Decoder API | Detection input and prediction output contracts | Decoder implementations, simulation internals |
| Facade | Finite canonical root contracts and direct component namespaces | Algorithms, duplicate models, universal errors, qualification plans |
| CLI | Argument parsing, file-role preflight, sink construction, rendering | Quantum algorithms |
| Ops | Tests, oracle, qualification, benchmarks, release operations | Product runtime behavior |

Detailed component contracts use [the component contract template](component-contracts.md).

## Permitted Dependencies

The current product graph is:

```text
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

stab-reference-decoder -> stab-decoder + stab-model + stab-records
stab-reference-noise-pass -> stab-analysis + stab-model
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-records + stab-decoder
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-bits
stab-analysis -> stab-model + stab-algebra
stab-model -> stab-foundation
stab-algebra -> stab-bits + stab-kernels-simd + stab-foundation
stab-records -> stab-bits + stab-foundation
stab-foundation -> no Stab crate
stab-bits -> stab-kernels-simd
stab-kernels-simd -> no Stab crate
```

`stab-foundation`, `stab-kernels-simd`, `stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, `stab-engine`, and `stab-decoder` are physical Cargo packages with one owner per capability. `stab-core` reexports common owned values and aliases `analysis`, `decoder`, and `execution` directly to their component crates. Low-level storage, codecs, traversal, algorithms, and extension contracts remain in their owners. Dependency arrows point from a consumer to its dependency:

```text
stab-foundation -> no Stab crate
stab-kernels-simd -> no Stab crate

stab-bits -> stab-kernels-simd
stab-records -> stab-bits + stab-foundation
stab-algebra -> stab-bits + stab-kernels-simd + stab-foundation
stab-model -> stab-foundation
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-bits
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-records + stab-decoder
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

ops -> product crates
product crates -X-> ops
```

`stab-foundation` exists so that `stab-model`, `stab-algebra`, and `stab-records`, which do not depend on one another, share one `MemoryBudget`, one `Cancellation`, one `Estimate`, one `ByteSpan`, and one `DiagnosticSeverity` instead of converting between look-alikes. Each of the three re-exports the items in its own signatures, so no other crate names `stab-foundation` directly. A refused memory budget is one value, `MemoryBudgetExceeded`, holding the required amount (exact or a lower bound) and the budget; each component's error describes what it was building in its own types, embeds that value, and answers `memory_budget_exceeded()`, which wrapping errors delegate to. The CLI prints its `--memory-budget` help and the `resource-limit-exceeded` code from that accessor and its own typed variants, never from message text.

The `stab-engine -> stab-analysis` edge carries three analysis algorithms and their error type, not gate vocabulary, which `stab-model` owns: detector-frame lowering decomposes `SPP` and `SPP_DAG` through `visit_decomposed_spp_instructions`, reference-sample Clifford kernels take each gate's local tableau from `single_qubit_clifford_for_gate` and `gate_tableau` because tableaus are `stab-algebra` values that the model cannot name, and sampled-flow checking removes noise with `circuit_without_noise` and resolves record indices with `flow_record_index`. Engine errors wrap `AnalysisError` from those calls.

The unpublished `stab-reference-decoder` proves decoder composition using only `stab-decoder`, `stab-model`, and `stab-records` at runtime; its analysis and engine dependencies are test-only experiment fixtures. The unpublished `stab-reference-noise-pass` proves circuit-pass composition using only `stab-analysis` and `stab-model`. Neither proof crate can depend on `stab-core`, CLI, ops, private modules, or unstable Rust features.

`just architecture::check` derives package identity, allowed product dependencies, binary targets, and protected package names from one product-contract table. It enforces every workspace edge, rejects product dependencies on operational crates or external packages impersonating reserved Stab product names, permits only the source-owned public-component edges required by each test-support proof, rejects any resolved `fearless_simd` or `fearless_simd_macros` dependency outside `stab-kernels-simd`, and requires every product manifest and both proof-crate manifests to declare the exact Rust 1.100 minimum. It also parses every GitHub Actions workflow and rejects mutable remote action and reusable-workflow refs; remote actions require full commit SHAs, Docker actions require `sha256` digests, and repository-local actions remain source-bound to the checked-out commit.

The checker classifies workspace packages as product, operations, or test support from their repository paths, resolves Cargo metadata with all features enabled so optional edges cannot hide, validates every workspace dependency edge, and rejects test-support dependencies on product or operations code except the exact public-component edges assigned to the two proof crates above. It retains resolved package identities for workspace and transitive dependencies, so a path, Git, or registry package that reuses a protected Stab package name cannot bypass the local dependency graph.

The checker parses facade and product Rust sources with `syn`. It requires the `analysis`, `decoder`, and `execution` crate aliases, rejects public facade modules, module path overrides, item-generating macros, exported macros, glob exports, and direct public definitions, and compares every common root reexport against `ops/architecture/facade-root-reexports.txt`. Feature-gate inspection follows nested and target-gated `cfg_attr` forms without treating comments or string literals as code.

The shared result-format corpus lives under `test-support/compat-corpus` and is available to product crates only as a development dependency. It is not a runtime architecture allowance.

The SIMD library belongs only to the mandatory `stab-kernels-simd` product crate, as recorded in [ADR 0008](adr-0008-unconditional-fearless-simd-kernels.md). Any product-to-ops edge, product runtime edge to test support, unapproved test-support upward edge, or normal, development, or build dependency on `fearless_simd` or `fearless_simd_macros` from another package fails the check, including renamed dependencies, because the rule runs on resolved Cargo metadata. Every unstable Rust `feature(...)` gate is rejected in every product crate and both proof crates, including target-gated and nested `cfg_attr` forms.

`just architecture::consumer-check` compiles and tests the standalone component-only (`stable`), facade (`facade`), and mixed direct-component (`mixed`) consumer workspaces under `test-support/consumers/` with the pinned Rust 1.100.0 toolchain; each declares the 1.100 minimum. It requires exactly one resolved `stab-kernels-simd` package in each graph, requires `stab-core` in the facade and mixed graphs, and rejects it in the component-only graph.

`just architecture::docs-check` uses `pulldown-cmark` with GitHub Flavored Markdown extensions to recursively validate repository-owned Markdown links. It derives same-file and cross-file heading anchors using GitHub's Unicode stripping and duplicate-suffix rules, treats heading text such as `{#custom}` literally instead of enabling non-GFM custom heading attributes, resolves local targets without permitting traversal outside the repository, reports all failures in deterministic source order, skips explicit external schemes, and excludes generated, build, vendor, and Git trees.

## Toolchain Boundary

The workspace builds with one pinned Stable toolchain, Rust 1.100.0, recorded in `rust-toolchain.toml`. Every product crate, including `stab-core`, `stab-cli`, and `stab-kernels-simd`, declares Rust 1.100 as its minimum supported version through `[workspace.package]`, and there is no Nightly package class.

No product crate may enable an unstable Rust feature; `just architecture::check` rejects any `feature(...)` gate in product and proof-crate sources.

Generic packed storage lives in Stable `stab-bits`, and quantum-specific Clifford and Pauli-word operations live in Stable `stab-algebra`. A `stab-algebra` Tableau stores its generator images as packed X and Z row planes with one sign plane, conjugates 64 rows per block so each generator row is read once per block, and uses `stab-bits` matrices for transposition, so composition, inversion, random sampling, and the stabilizer solver are word-parallel with no width cap; values built from a requested size are admitted against the shared `MemoryBudget` of `stab-foundation`. Whole-slice XOR in `stab-bits` and Clifford and Pauli right multiplication in `stab-algebra` always run through the `fearless_simd` kernels in `stab-kernels-simd`, which select the instruction set once per call; the remaining word loops, including the single-block XOR and masked XOR, are scalar. The Pauli product kernel has a phase-only variant for Tableau row products, which skips the non-identity tracking that `PauliString` keeps, and on 128-bit levels a product of at least 64 words runs six-word blocks of one `u64x4` and two scalar words, because the vector units alone bound that loop (measured 25 percent faster on the AArch64 host's Cortex-A725 cores and neutral on its Cortex-X925 cores). Prepending CX in either direction, CZ, XCX, or SWAP, with any local signs, rewrites two rows with straight-line code; any other two-qubit gate rewrites its four rows in place with a shortest row-product program taken from a compile-time table, so no two-qubit prepend copies a row. A single `Tableau::append` visits every generator row, which is inherent to the row-major layout: at 5000 qubits it costs about 4 ns per row, 31 µs in a shared-host diagnostic, the same as a bare loop over that layout; callers that append many gates use `TableauAppender`, and `Tableau::prepend` is the word-parallel left-action route. The kernels accelerate only measured leaf operations. Circuit sampling, detection sampling, measurement-to-detection conversion, and sweep correction all run one generic Pauli-frame batch with scalar block operations, 256 lanes and one `BitBlock` per plane by default; circuit-sampling and detection-sampling sessions may instead be created with a 64-lane `LaneWidth`, one `u64` per plane, which the CLI selects for small requests ([ADR 0003](adr-0003-typed-batch-families.md)). A materializing session keeps one plane per measurement (and per detector) for every lane of a batch; a streaming run (`SamplingPlan::streaming_run`, `DetectionSamplingPlan::streaming_run`) hands the frame's planes over in windows of about 1 MiB, keeps only the planes a later classical control, detector, or observable can read, and transposes each window into shot-major records of the lanes it delivers, so its storage is the frame, one window, and `min(shots, lane_width)` packed records, and its output equals a materializing session's first run. The CLI's `sample` and `detect` use a streaming run for 1 to 63 shots, and for larger requests whose output planes (measurements, or measurements and detectors) fill at least 8 MiB per batch unless they write PTB64: a streaming run keeps at most one lane width of records, about as many bits as one batch's output planes, but no measurement planes beside detector records, no planes of undelivered lanes, and no encoder transpose scratch, and it transposes each window while the window is in cache, which measured up to 20 percent faster on records of millions of bits and up to 4 percent slower on records whose planes fit the caches, where materializing costs little memory; PTB64 is plane-major, so it materializes and the encoder copies the frame's plane words. The memory floor of a streaming run is therefore one lane width of complete records, by choice: going lower needs narrower lanes, which would change seeded output, or compressed records, and streaming PTB64 output measured 10 to 28 percent slower than materializing it. Measurement-to-detection conversion without sweep correction windows the same way: a `MeasurementToDetectionSession` transposes each window of a batch's measurement columns into 256-lane planes, resumes the conversion program over it, keeps only the planes a later detector or observable reads, and delivers shot-major detector and observable records of the batch's shots, so a batch costs one window plus its own records instead of one plane per measurement and detector; with sweep correction, whose frame replays every measurement plane, it keeps the full-width planes. Whole-batch conversion under sweep correction is accepted for that reason rather than windowed. The CLI's `m2d` reads a first batch of as many records as fit 1 MiB, at least one, then 64, then 256, so an input of a few records of a very long circuit holds memory for those records rather than for 256 of them. The cached reference sample is packed one bit per measurement. Lane rows of 1024 driven by the slice kernels were measured and not adopted because the end-to-end cases did not improve, so the engine registers no SIMD sampling backend, and a 128-lane width was measured and not adopted because it won only in narrow shot-count bands on large frame-dominated circuits. Every shot count, including a single shot, takes this one route: the whole circuit is parsed, validated, and lowered into the resolved program, and its storage is admitted against the memory budget, before the first shot runs. Stim instead interprets `sample --shots 1` with the reference sample directly from the source text on a tableau simulator, without building a circuit. Stab deliberately has no such second executor, because it would duplicate every gate, noise, and control-flow semantic outside the resolved program, give one shot a different seeded stream from the first shot of a larger request, and learn its qubit and measurement counts only while running, after output has begun, instead of validating and admitting first. The accepted cost is a fixed parse-and-lower time and a peak of the parsed circuit plus the lowered program on inputs of very many short instructions: a shared-host diagnostic on 1.5 million short instructions measured 0.14 s and 138 MiB against Stim's 0.08 s and 4 to 6 MiB for one shot with the reference sample, and 0.12 s against 0.13 s without it, where Stim also builds its circuit.

Strict `01`, `b8`, `r8`, HITS, DETS, and PTB64 codecs live in Stable `stab-records` behind one public `RecordFormat` type. Materialized readers and the one packed `RecordStreamReader` accept all six formats. The stream reader decodes straight into the words of a reusable `PackedShotBatch`: sparse formats set only hit bits, `01` packs eight bytes per step, `b8` copies bytes, and PTB64 transposes whole groups for row consumers or yields each 64-record group directly as a `BitPlaneBatchView` via `read_ptb64_group` without transposing rows. Canonical HITS and DETS lines are framed and decoded in one byte loop that resumes where it stopped when a line spans a buffer refill, and any other line is redecoded by the shared grammar, which owns every diagnostic. `r8` records decode run bytes straight into the packed row as they arrive, carrying the bit position across refills, so an `r8` record of any width never spans the buffer and a failure inside a partly read record is reported again by every later read. It reads through one buffer that grows only while a `b8` record, a PTB64 group, or a text line spans it, and never past one byte beyond the text record cap, allocates record storage fallibly and only after input proves the first record exists (the first `r8` record keeps its hits in storage that follows the input until it completes), and delivers every record decoded before a failure ahead of that failure. `RecordStreamStorage` bounds what one reader retains for a format, width, and batch size before the reader exists (its batch, its PTB64 group, the first `r8` record's hits in at most one row, the transport buffer around one worst-case `b8` record or PTB64 group, and the fixed initial buffer for `r8`), and bounds a text reader's buffer by its record cap, `RecordStreamStorage::max_text_record_bytes` being the largest cap whose buffer fits a budget; `RecordEncoder::shot_major_scratch_bytes` bounds an encoder's group planes and pending output (`shot_major_scratch_bytes_for_records` for a known record count) and `RecordEncoder::bit_plane_scratch_bytes` its transposed 64-shot blocks, so `convert` admits its reader and encoders against its memory budget before reading and `sample`, `detect`, and `m2d` admit their output encoders before their sessions or records. Its framing options carry the `m2d` and `sample_dem` replay diagnostics (line offsets, newline and length checks, `b8` truncation, empty zero-width sweeps, set-semantics HITS), so the CLI contains no format-specific decode loop. Every product output route encodes through one `RecordEncoder`, which takes packed records as single records or as column groups that are each shot-major records or bit planes of any shot count, and drains the encoded bytes into any `io::Write`. Bit planes are the layout materializing sessions produce; the encoder copies their words straight into PTB64 groups and transposes one 64-shot block per column group for every other format, so no materializing engine path re-chunks or transposes planes before a sink sees them, while a streaming run, and measurement-to-detection conversion without sweep correction, deliver the shot-major records they transposed window by window ([ADR 0003](adr-0003-typed-batch-families.md)). A record part wider than 65,536 bits is encoded and drained in segments, so one wide record never holds its whole encoding in memory. It owns record separators, DETS namespaces, B8 tail masking, and PTB64 grouping; sparse formats visit only set bits, and its retained state between calls is at most one incomplete PTB64 group, which callers must reject at finalization. The per-bit `MeasureRecordWriter` remains as the Stim writer-contract mirror and is fallible because PTB64 requires complete 64-record groups.

Dense HITS and DETS input is slower to read than the same records as `b8` because the text carries a decimal index per set bit: a shared-host diagnostic at about 10 percent density measured 2 times (100-bit records) to 14 times (1,000,000-bit records) the `b8` read time, and no more than `b8` at about 0.1 percent density. That is a property of the formats rather than of the reader, which is within 5 percent of a bare digit loop on wide records; an 8-byte digit parser and a loop with fewer bounds checks both measured slower and were not adopted.

Materialized and streaming readers apply HITS and DETS token events directly, so duplicate-heavy input cannot make their scratch grow beyond the declared width. `MeasurementCodecSink`, `DetectionCodecSink`, and `DemSampleCodecSink` are explicitly in-memory sinks over `RecordEncoder`, so their encoded output grows with requested output bytes while their additional scratch stays bounded by one active batch or PTB64 group.

DEM sampled-error replay in `stab-engine` takes batches of at most 64 records and replays each batch along whichever of two paths a cost model estimates to be cheaper; both produce identical records.
Error-major replay transposes the batch into one 64-record word per unrolled error and walks the compiled program once, XORing each nonzero word into the words of its detectors and observables, skipping repeat iterations whose words are all zero, and iterating a flat repeat body without descending into it per iteration, so its cost per batch follows the unrolled error count and not how many errors each record sets.
Per-record replay scans each record for set bits and locates each set error through the per-block error spans: the ordinals of one record arrive in increasing order, so each repeat depth gallops forward from the span of the previous error and falls back to a binary search when that span no longer precedes the ordinal, and an error inside a `repeat` resolves its iteration and offset by division by the body's error count, so a record costs its scanned words plus a lookup per set error that is constant within one segment and logarithmic otherwise.
The cost model weighs the unrolled error count against the record count, the record width, and the set errors of the batch's first record, so dense records and narrow models replay error-major and sparse records of wide models replay per record.
A caller that validates a whole replay input before writing any output can keep the validated records in a `DemReplayRecordStore` budgeted at what a replay session leaves of the same memory budget—either as error-major bit planes or as sparse `u32` set-error ordinals—and replay them without decoding the input again; regular `b8` and PTB64 files whose byte length already proves the requested shot count stream once without a store, and PTB64 groups replay straight from `BitPlaneBatchView` without a shot-major transpose.
When the next batch would exceed the store's budget the store becomes full rather than releasing: it keeps the whole batches it already holds, still charged against the budget, and declines that batch and every later one.
A dense first batch whose error-major words for every expected record do not fit, and whose set errors projected over every expected record would not fit as sparse ordinals either, reserves error-major words for the whole batches the budget admits when that layout costs fewer bytes per record, so a dense replay that just misses retention also keeps a prefix.
`sample_dem` records the `RecordStreamReader::consumed_offset` before each batch it offers, and finishes validating the rest of the input.
When the store kept every record, delivery reads the store alone and never touches the input again, so a pipe replays as well as a file.
When records were declined, `sample_dem` seeks the replay file to the first of them, so delivery replays the kept prefix from memory and decodes only the records after it through a reader created with `RecordStreamReader::starting_at_offset`, whose record counts and diagnostic offsets stay those of the whole file; a non-seekable input fails that seek before any output exists, with a diagnostic that asks for a regular file or a larger `--memory-budget`.
While validating, `RecordStreamReader::skip_records` checks wholly buffered canonical record-framed `01` lines (exactly the record width of `0` and `1` bytes, then `\n` or `\r\n`, within the record cap) without packing them and hands any other line to the full grammar, which owns every diagnostic.
Decoding the records after the kept prefix a second time is deliberate: validating the whole input before the first output byte means a malformed replay leaves no partial output, and writing each output in place keeps its symlinks, hard links, mode, and ownership, which staging through a temporary file and renaming would replace, while standard output and pipes cannot be staged at all.
The accepted cost applies only to variable-width inputs (`01`, HITS, DETS, and `r8`) whose records do not all fit the budget, a shared-host diagnostic of 10 to 17 percent on sparse HITS, DETS, and `r8` when nothing is retained, and a larger `--memory-budget` removes it.
The replay input must not change while the command runs: the kept prefix is not compared with the file again, so a file rewritten between the passes is replayed from the old prefix and the new remainder, and the second pass still decodes through the same validating reader, so such a file ends in an ordinary replay diagnostic or well-formed output, never a panic.
Detecting the change reliably would mean reading the prefix again, which is the work the store exists to avoid, and whoever can rewrite the input could have supplied the other content from the start.
`sample_dem` admits its replay reader's storage (`RecordStreamStorage` for the format and width, plus a text reader's line buffer at a cap that receives a quarter of what the reader and the smallest replay session leave) through `DemSamplingPlan::admit_replay_input` and holds it before reading, so the replay session and the store are budgeted beside the reader rather than from the same unreduced budget.

`stab-kernels-simd` has no Stab dependency and accepts only raw word slices.

Product crates must compile on the pinned Stable toolchain without enabling unstable features.

## Public Facade

- The facade root contains only the mechanically checked canonical contracts listed in `ops/architecture/facade-root-reexports.txt`. The inventory is limited to common values, their directly coupled errors, and sink/session contracts.
- `stab_core::analysis`, `stab_core::decoder`, and `stab_core::execution` are direct crate aliases with the same type identities as their owners.
- Sampling request estimation is engine-owned and available as `stab_engine::estimate_sampling_request` or `stab_core::execution::estimate_sampling_request`.
- Low-level storage and kernels use `stab-bits`; codecs and traversal use `stab-records` or `stab-model`; pure algorithms and extension contracts use `stab-analysis`; execution plans use `stab-engine`.
- `advanced`, `experimental`, duplicate owner-shaped modules, universal facade errors, and backend placeholders are absent.

The root inventory is intentionally smaller than the union of component APIs. Convenience does not imply facade ownership.

## Resource And Diagnostic Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.
It does not mean bounded CPU time, which belongs to the caller.

Every materializing or executing operation admits its conservative retained storage against one typed `MemoryBudget` before allocating, advancing randomness, or writing output, and reserves fallibly after admission.
The budget defaults to one gibibyte, is caller-selectable through each operation's limits type, and is exposed on the CLI as the one Stab-native `--memory-budget` option.
On the CLI the budget covers the whole command: each stage admits against it less the bytes earlier stages still hold, and the CLI drops input text, parsed models, and side inputs as soon as no later stage reads them and returns their bytes to the budget, as the [resource policy inventory](resource-policy-inventory.md#command-level-cli-budget) details.

Format and representation invariants and the shared 256-level repeat-nesting envelope are not configurable resources.
`stab-model` parses and visits nested DEM repeats iteratively, while the DEM consumers in `stab-engine` (the sampling plan) and `stab-analysis` (flattening, error search, and SAT export) recurse once per repeat level by design: each checks the depth against that envelope first and refuses a deeper model, which only the Rust API can build, with a typed error, so their recursion is bounded at 256 frames.
Coordinate queries on a circuit or DEM keep a per-query cache of each repeat body's detector count and coordinate shift, which makes them linear in nesting depth; the cache is deliberately not charged to a memory budget, because these read-only queries take none, it is freed when the query returns, and it holds at most one count and one shift vector per repeat body of a model already in memory, each vector no longer than the widest coordinate shift nested in that body.
The engine's lowering counts its own admission quantities instead of reading `FoldedCircuitSummary`, because it needs a running measurement count in source order to validate record lookbacks, the sweep-bit and simulated-qubit maxima, and counts that depend on the lowering itself; the detecting-region walk in `stab-analysis` takes its entry counts from the summary and asks each repeat body it enters for its own tick count, because the summary holds whole-circuit totals and cannot name one body.
Two small holds stay outside the budget by choice: standard output's 64 KiB buffer, which exists before the budget is known, and the old target buffer that a fused instruction's reallocation briefly keeps beside the new one, since charging both would refuse circuits whose retained size fits; `m2d`, which cannot know its record count up front, is charged the worst-case pending-output bound.
Explicit search budgets with budget-exhausted results remain separate caller policy.
Count caps that only proxy memory and fixed work caps are not used; a compact input that fits the budget runs for as long as its work takes unless the caller cancels it.
Long-running library operations accept one `stab_model::Cancellation` signal on their limits or options value, or for circuit passes on the pass context, poll it at bounded work intervals inside repeat interpretation, search, and between batches, and stop with a typed `Cancelled` outcome; decoder sessions observe the same signal type between records.
Frame batches with their reference flips and output-plane clearing, detector conversion, streaming window hand-overs, partial-delivery lane shifts, and the reference simulator space their polls by estimated lane-word or tableau work rather than by instruction count, walking wide target lists in charged chunks, so one instruction of any width stays cancellable within about a millisecond on the development host without reading a clock.
Frame sessions reserve their planes at creation and write them in the polled reset of each batch, and a streaming run or windowed conversion zeroes its shot-major records in chunks charged by word through `BitMatrix::zeros_chunked`: creating a streaming run polls the plan's signal and returns the typed `Cancelled` error, and a windowed conversion polls the session's signal when a batch first needs larger records.
Creating a measurement-to-detection session with sweep correction, copying each batch into its frame planes, and the reference and frame XOR passes of a conversion poll the same way, and a folded reference repeat is written a word at a time and charged per chunk of words.
Two stretches stay unpolled by design: dropping a session's storage, which returns memory and has no state worth resuming, and the caller's sink, whose time is the caller's.
Three short passes are also unpolled because their width is the sweep-bit or observable count, which is thousands at most in practice and passes a millisecond only near 150,000: filling the sweep planes when no sweep input is given, sizing and clearing the observable planes of a streaming detection run and of a windowed conversion, and the transpose of one 64-shot column of a measurement batch.
Polls are spaced by estimated work rather than by reading a clock, so their wall-clock spacing varies with the host.
A circuit-sampling or detection-sampling session delivers a batch only once it is complete and rewinds its random stream when cancellation interrupts a batch, and a lane shift that cancellation interrupts keeps the planes it already moved and finishes before the next delivery, so cancellation, wherever it is observed, returns the resumable `Cancelled` status and a resumed session continues the uninterrupted seeded output byte for byte; only a sink failure or an internal invariant violation poisons a session.

The [resource policy inventory](resource-policy-inventory.md) classifies every remaining limit constant and records each removed one.

### Path Trust Boundary

A path named on the `stab` command line (`--in`, `--out`, `--obs_out`, `--circuit`, `--dem`, and the other file flags) carries the authority of the caller who typed it, and the CLI opens it as given.
The CLI does not reject `..` components, symlinks, devices, or locations outside some directory, because it has no intended output root: the caller can already read or write the same path with any other tool, `/dev/stdout` is itself a symlink, and writing through a symlinked scratch directory is ordinary use.
Pinned Stim behaves the same way, so this is not a divergence.
What the CLI does check is that the caller's own flags do not conflict: no output may name the same regular file as an input or another output, an existing output is written in place so its link, mode, and ownership survive, and no output is truncated before every file role has been checked.
The rule that rejects path traversal, unsafe components, unexpected symlinks, and writes outside an output root applies to every path that Stab derives from input content, such as an archive entry, a manifest row, or a workspace file.
No product crate derives a path: the library crates take readers, writers, and byte slices, never paths, so an embedding application owns its own path policy, and only `ops/` tooling reads manifests, archives, and workspace contents.
A service that runs `stab` on behalf of an untrusted user must confine the process with the operating system, since a path filter inside the CLI would not be a sandbox.

Domain errors retain typed context. Applications and the CLI compose them into structured diagnostics only at their workflow boundary.

Human rendering is for people.

Stable diagnostic codes and JSON rendering are for tools and agents.

## Extension Policy

The first supported extension seams are:

- typed circuit passes that admit conservative output projections before lowering and return validated Stim-compatible circuits;
- packed measurement and detection sinks;
- decoder sessions consuming detection batches;
- unconditional leaf kernels with per-call instruction-set selection outside word loops.

Dynamic Rust libraries, runtime gate registration, serialized executable plans, and unimplemented backend placeholders are forbidden.

An extension seam is accepted only after a separate crate uses it without private or operational APIs.

The common circuit-pass executor admits the folded input before dispatch, admits each pass's conservative output projection, whose projected payload is charged to the caller's memory budget and whose repeat nesting is checked against the caller's nesting limit, before proportional lowering allocation, and validates the returned `Circuit` against both the caller policy and that projection. Projected payload excludes allocator metadata and spare collection capacity, so it is not a resident-memory claim. Pass-specific options, reports, and diagnostics remain associated types. Since the model dialect is closed, a research operation that cannot lower to ordinary Stim-compatible gates is rejected instead of registering a runtime instruction.

Requirements for a future process-isolated decoder are documented in [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md); Stab 0.2 does not implement that transport.

## Decision Records

- [ADR 0001: Stim Dialect Remains Closed](adr-0001-closed-stim-dialect.md)
- [ADR 0002: Plans, Sessions, And Sinks](adr-0002-plan-session-sink.md)
- [ADR 0003: Typed Batch Families](adr-0003-typed-batch-families.md)
- [ADR 0004: Diagnostics And Resource Policies](adr-0004-diagnostics-and-resources.md)
- [Resource Policy Inventory](resource-policy-inventory.md)
- [ADR 0006: Decoder And External Extension Boundaries](adr-0006-decoder-extension-boundaries.md)
- [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md)
- [ADR 0007: Product Dependency Graph](adr-0007-product-dependency-graph.md)
- [ADR 0008: Unconditional Leaf Kernels On Fearless SIMD](adr-0008-unconditional-fearless-simd-kernels.md)

## Change Rules

Architecture changes require:

1. an updated decision record or a superseding decision record;
2. architecture dependency checks;
3. semantic tests for the moved contract;
4. focused benchmarks for performance-sensitive boundaries;
5. synchronized public and generated documentation;
6. milestone audit and full code review before formal evidence.
