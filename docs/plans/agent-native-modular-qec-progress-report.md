# Agent-Native Modular QEC Progress Report

Current as of 2026-07-27.

## Status

- A0 architecture contract and baseline: complete.
- A1 logical ownership and dependency enforcement: complete.
- A2 diagnostics, resources, fingerprints, and capabilities: complete at clean source revision `7b6c592b08f6a24d31a0673588dce7525b1c02c9`.
- A3 stable packed records and codecs: not started.
- Formal correctness and performance evidence for the current post-A1 inventories: not started.

The accepted pre-refactor formal evidence remains bound to clean revision `68d107a42f655254f31628f0cbedc55479f6c0f3`.

## A0 Foundation

| Commit | Purpose |
| --- | --- |
| `4fc7c505` | Define the rationalized migration plan, target graph, ADRs, component contracts, and active execution goal. |
| `b3104e69` | Add the Cargo-metadata architecture checker, `just architecture::check`, and CI enforcement. |
| `c4c7555c` | Separate historical qualification checkpoints from the new architecture program. |

The plan accepts the external review's compiler-style direction while rejecting unnecessary abstraction. Stab keeps a closed Stim dialect, a private executable IR, operation-specific policies, focused batch families, static hot-loop dispatch, and public extension traits only after two real implementations prove a useful common contract.

## A1 Implementation

| Commit | Purpose |
| --- | --- |
| `01ce5669` | Remove the algebra layer's dependency on the Stim gate model and establish semantic gate adapters. |
| `220d27cd` | Move simulator-backed sampled-flow checks into execution ownership. |
| `e5df8807` | Move determined-measurement counting into the circuit model facade and execution implementation. |
| `102eb68a` | Validate the complete dependency graph, including optional features. |
| `3097f42b` | Make performance regeneration validate the normalized inventory it actually retains. |
| `aa93a9d4` | Test every ordered product dependency edge for normal, development, and build dependencies. |
| `05a69aa3` | Establish public `analysis` and `execution` namespaces, model-owned folded DEM traversal, consumer-owned traversal policies, compatibility adapters, and exact qualification aliases. |

Gate tableau, flow, unitary, and decomposition semantics now have one analysis owner. Pure circuit and DEM transforms are analysis-owned. Reference sampling, compiled sampling, determined-measurement counting, and sampled-flow checks are execution-owned. Existing root types, root functions that predated A1, and inherent methods remain compatible adapters.

Folded DEM traversal remains model-owned and crate-internal. Its compact block identity is a deterministic traversal-local preorder index rather than a pointer address. Search, SAT, and ErrorMatcher retain their own policy caches so traversal does not absorb consumer-specific semantics.

## Review Feedback And Repairs

The milestone audit found that representative forbidden-edge fixtures did not prove the stated requirement to reject every forbidden edge. A1 now table-drives all product package pairs and all dependency kinds against the source-owned graph.

The audit also found that the human migration ledger omitted execution-owned root movements. The ledger now covers `CompiledSampler`, `ReferenceSampleTree`, determined-measurement counting, and sampled-flow checking in addition to circuit, gate, and DEM adapters.

Full-code-review found that the first namespace implementation accidentally added new root free functions for gate semantics, circuit transforms, and reference sampling. Those root additions were removed. Namespace functions now map directly to the pre-existing inherent-method evidence parents, avoiding an unplanned facade and qualification lifecycle.

Exact public-API alias validation was added instead of relying on parent-name similarity. The checked ledger rejects self-aliases, chains, stale paths, feature mismatches, undeclared aliases, and attempts to use a parent alias to authorize an undeclared child.

The audit exposed three genuine specification gaps: namespace completeness, folded-visitor visibility, and diagnostic benchmark provenance. The A1 contract now enumerates required namespaces, defers detection and DEM sampling namespace completion to A5, keeps folded traversal crate-internal until A6, and defines the exact diagnostic benchmark policy.

## A1 Qualification State

- Correctness schema: 4.
- Correctness inventory: 2,886 upstream cases, 2,159 public API items, and 1,759 evidence parents.
- Correctness digest: `eef15f812b10889de6572a25ec8bc3322b7dd075f15b8a470bab907277f7c383`.
- Performance schema: 3.
- Performance inventory: 127 checklist rows, 2,159 public API items, 169 groups, and 161 inherited manifest decisions.
- Performance digest: `1b427ef982217037371714676f3572386f9d005b016e17c2fd2afd25dc2ba6ea`.

These identity changes make the previous formal evidence historical. No A1 result is being promoted as source-current formal qualification.

The first A2 diagnostic slice advances the current inventory to 2,218 public API items and 1,763 evidence parents. Its correctness digest is `2fc7cc31e97de88a6c2707317b9c01ab0bf03e55ff0f7aa743f4918679021fee`, and its performance digest is `87a12f0778c38ba3ee8ec85571ca2cb9b1946c9289488b7c4b68408c3d9d644e`. The A1 identities above remain the exact closure checkpoint rather than being rewritten as if A2 had been part of A1.

The typed-context and CLI JSON slice advances the current inventory to 2,237 public API items and 1,772 evidence parents. Its correctness digest is `b8ee2e2daa6a35e52d54713505c44ba08a1cd35a21a39ca77be60321bd55ea1c`, and its performance digest is `95bfb5065c302569870ccc8fcd666268a315b6a4fb311a154be8df6c72466584`. These identities supersede the first-slice identities for current work without changing that committed slice's historical record. Typed context construction remains domain-owned so external callers cannot pair a stable code with an incompatible context variant.

## Verification

The A1 closure passed:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `RUSTDOCFLAGS='-D warnings' cargo doc -p stab-core --no-deps`
- `cargo test -p stab-architecture --quiet`
- `just architecture::check`
- `just qualification::correctness-check`
- `just qualification::correctness-regenerate --check`
- `just bench::qualification-check`
- `just bench::qualification-regenerate --check`
- `just qualification::status --check`
- `just bench::smoke`

The primary diagnostic comparison is `target/benchmarks/a1-final-primary-compare`. It used accepted clean baseline `target/benchmarks/q8-final-f465b6f-primary-baseline/baseline.json`, one warmup, one measurement run, source revision `102eb68a0cad1211cda51b1b31fac926da919755`, and `local_modifications=true`. It measured 86 rows: 77 passed and nine were explicitly not comparable. The maximum comparable ratio was `1.25x`. This is diagnostic evidence only.

## A2 Sequencing Feedback

The original plan asked A2 to create a `PlanFingerprint` containing a selected backend, but backend selection is introduced in A4. Creating that identity in A2 would either invent a placeholder backend contract or require later semantic replacement.

A2 therefore owns `ModelFingerprint` and backend-neutral `CompilationRequestFingerprint`. A4 completes `PlanFingerprint` after compilation can bind the selected backend and executable-contract identity. This keeps fingerprints honest and avoids designing around state that does not yet exist.

The first A2 slice is the result-format diagnostic nucleus because the existing byte-oriented text grammar already has strong pinned-Stim coverage. It can establish stable codes and exact byte spans with low behavioral risk before circuit and DEM parsers acquire position-aware source cursors.

Implementation has started with a typed `FormatError` payload, stable `FormatErrorCode` values, `DiagnosticSeverity`, overflow-aware `ByteSpan`, and `FormatErrorContext`. The existing `CircuitError` human prefix is preserved, while `CircuitError::format_error` exposes the typed payload. Exact 01, HITS, and DETS span selectors own this first slice; packed-format, circuit, and DEM spans remain in A2.

The CLI now has an additive global `--error-format=human|json` mode. Human mode remains the default. JSON mode writes one schema-version-1 object per warning or error, preserves warning-before-error order, derives absolute offsets for line-streamed `m2d` and `sample_dem` inputs, and keeps alias preflight and original writer-error precedence intact. Serde remains private to `stab-cli`; `stab-core` does not acquire a serialization dependency. Schema-v1 includes an empty labels array because no current producer has a meaningful secondary label, avoiding a speculative public label API while keeping the wire field stable.

Clap still owns argument validity. A private raw probe selects JSON for parse-time failures only when exactly one valid JSON request occurs before `--`; malformed, duplicate, and post-`--` spellings do not bypass or reinterpret Clap. Help and version remain successful human stdout operations even when JSON diagnostics are requested.

Packed diagnostics are implemented through shared cold-error validation instead of independent reader patches. `b8` and `ptb64` share exact byte-multiple and minimum-length failures, while one inlined zero-allocation `r8` record decoder feeds materialized, dense, packed, and sparse consumers. This prevents diagnostic drift without adding a token vector or dynamic dispatch to successful reads. The materialized `r8` adapter retains one allocation per returned record and does not allocate an unused trailing record.

The packed-format slice advances the current inventory to 2,252 public API items and 1,776 evidence parents. Its correctness digest is `cf4cc26432e4b84e45815f3e7037043e53b50004e7eb31584285ba16222fad8e`, and its performance digest is `98c3ab4c8ea6125633cf05cc0492d333b7f6d8f7e877d0e03e12acfcea0a8c38`. The prior diagnostic identities remain historical checkpoints for their committed slices.

The first event-at-a-time `r8` decoder probe is retained at `target/benchmarks/a2-packed-m8-r8`; it failed the unchanged gate at `1.310x` Stim on the sparse-per-10 filter and was rejected. The replacement inlined per-record decoder at `target/benchmarks/a2-packed-m8-r8-record-decoder` records `0.432x`, while `target/benchmarks/a2-packed-m8-b8` records `0.931x`. The contract-only PTB64 probe records 10.30 billion bits per second at `target/benchmarks/a2-packed-m8-ptb64`; no Stim ratio is claimed. Public CLI reports record `0.314x` for `b8 -> 01` and `0.276x` for `ptb64 -> 01` at `target/benchmarks/a2-packed-m7-b8-to-01` and `target/benchmarks/a2-packed-m7-ptb64-to-01`. The allocation-enabled `r8` diagnostic at `target/benchmarks/a2-packed-m8-r8-allocations-enabled` records 12,288 allocation bytes, zero resident-memory delta, and `0.440x` Stim. All reports use committed source revision `66239b60091dc6f489fa9780ab1def62e91b901e`, `local_modifications=true`, and the accepted clean `q8-final-f465b6f` primary baseline, so they are diagnostic and non-promotable.

The first resource-policy slice moves circuit and DEM source-line and repeat-depth admission into model-owned `ParseLimits`. Existing parse entry points use defaults of 1,000,000 physical lines and 256 repeat levels. `SourceLineLimit` permits explicit tighter or looser line budgets, while `RepeatNestingLimit` may only tighten the shared non-overridable 256-level recursive safety envelope. Named quantities prevent constructor argument swaps. The policy intentionally has no target-ID, numeric-width, detector-ID, PTB64-grouping, generator-domain, or algebra-limit fields because those are semantic or representational invariants rather than configurable safety budgets.

`ResourceEstimate` reports only cheaply knowable facts. Text input bytes and physical lines are exact; expanded operations, folded traversal, scratch, resident memory, output, and work units remain unknown instead of being guessed. The upper-bound variant remains vocabulary for a later real estimator and is not promoted through a manufactured assertion. `ResourceLimitError` stores one private typed cause from which both the established human message and stable operation/resource JSON context are derived.

The pre-commit independent review found two defects in the initial implementation. Circuit capacity estimation scanned and preallocated from the complete hostile input before line admission, and an unrestricted repeat override could accept models outside the 256-level contract still enforced by downstream recursive consumers. Circuit and DEM preallocation now inspect a bounded sample and cap capacity by admitted lines; allocation instrumentation proves rejected trailing input cannot increase parser allocation. Exact generated tests prove acceptance at one million lines and 256 levels plus first rejection. The broad `resource-parser-input-admission` parent remains planned because this slice does not yet introduce a public input-byte policy.

This corrected slice advances the current inventory to 2,388 public API items and 1,785 evidence parents. Its correctness digest is `74533261b104c975cbce766ed081944fe8b6a87acafd05e8f24cfba3830d3562`, and its performance digest is `cec55c25ec15a5b2a5e93a94323c04a2a0b6d8ae6cd821c29cd2d6d9c8a60da2`. The API count includes variants and derived trait surface mapped to semantic parents; it does not create per-item runtime tests or benchmark products.

The dirty-worktree parse diagnostic at `target/benchmarks/a2-parse-policy-m4-circuit-parse` records the comparable sparse circuit parse at `0.917x` Stim. `target/benchmarks/a2-parse-policy-m10-dem-parse` records the contract-only deterministic DEM parse at 150 million input bytes per second and makes no Stim-relative claim. Both use committed source revision `c5ce55069103fcfea84f45a9f464886dee046c52`, `local_modifications=true`, the accepted clean `q8-final-f465b6f` primary baseline, one warmup, and three measurement runs.

An attempted mixed parse-and-estimate report is preserved at `target/benchmarks/a2-parse-policy-m4-circuit-parse-estimate` but is review-rejected. Adding an unmatched estimate submeasurement caused the report's headline Stab median to describe estimation while its ratio source still described parsing, which is scientifically ambiguous even though the paired parse ratio remained valid. The benchmark-runner change was removed. Estimate timing will receive a separate operation-level diagnostic when `stab inspect` exposes the estimate in the later A2 capability slice.

After bounding parser preallocation by the admitted line budget, `target/benchmarks/a2-parse-policy-bounded-preallocation` records the comparable sparse circuit parse at `0.943x` Stim. It uses the same committed source revision, dirty-worktree status, accepted clean baseline, warmup, and three-run protocol as the earlier parse diagnostic. The safety repair therefore remains inside the unchanged `1.25x` gate; no threshold, waiver, or benchmark row changed.

The first model-fingerprint implementation hashed canonical `.stim` and `.dem` printer output. External review correctly rejected it: the Stim-compatible printer rounds `X_ERROR(0.12345641)` and `X_ERROR(0.12345649)` to the same text, so semantically distinct models collided, printer changes could silently redefine schema one, and hashing required a model-sized temporary string. This rejected implementation was never committed.

The replacement adds a schema-one SHA-256 identity over an explicit model domain, big-endian schema, dialect discriminator, and streaming structural encoding. It length-frames sequences and UTF-8 strings with architecture-independent values, discriminates every model variant, preserves exact finite `f64` bits, and normalizes only signed zero. Comments, accepted gate aliases, case variants, whitespace, and line endings therefore normalize through parsing without coupling identity to presentation precision.

Rich frozen circuit and DEM vectors cover repeats, Unicode tags, every instruction and target variant, high-precision probabilities, and signed zero. Their expected digests were reconstructed independently with Perl `Digest::SHA` and binary `pack` operations from the published schema. The test also retains the exact rounded-printer collision as a negative regression, so it does not merely compare the implementation with itself.

Final review found that the first structural implementation still used recursive calls and that its compact prose was insufficient for independent schema reproduction. The encoder now uses an explicit traversal stack: the root plus all 256 parser-admitted repeat levels remain inline with zero heap allocation, while deeper programmatically constructed trees spill only additional traversal frames instead of consuming the call stack. [Model fingerprint schema version 1](../architecture/model-fingerprint-schema-v1.md) now normatively fixes every primitive, field order, discriminator, frozen vector, resource rule, and schema-evolution requirement.

No legacy benchmark row is added for this Stab-only primitive. Fingerprint timing will be measured as an explicit suboperation of the source-owned `stab inspect` benchmark later in A2, outside execution timing and without inventing a Stim-relative comparator. This follows the qualification-economy decision to stop growing overlapping benchmark ledgers while still measuring the public agent workflow.

The model-fingerprint slice advances the current inventory to 2,415 public API items and 1,788 evidence parents. Its correctness digest is `a13218df4789cb139c80eb8d6dc54ecd7529d40e450def66cd5c54ba9d615b7e`, and its performance digest is `b131a185236dff46ee290cfc4861f95b8401cb7700087c56900c58bc51c717cd`. The three evidence parents separately own the structured value contract, circuit canonicalization, and DEM canonicalization. Only circuit and DEM fingerprint generation remain future performance candidates; fixed-size metadata accessors are explicitly not independent performance products.

The CLI-boundary diagnostic comparison is `target/benchmarks/a2-json-cli-overhead`. It uses accepted clean baseline `target/benchmarks/q8-final-f465b6f-primary-baseline/baseline.json`, committed source revision `6aad05b8a2c257e8db857653d5837eef89dca0ad`, `local_modifications=true`, one warmup, and three measurement runs. The public `convert` path records `0.273x` Stim and the primary repetition-code sample path records `0.004x` Stim. Both pass the unchanged `1.25x` parity threshold, but this dirty-worktree comparison is diagnostic only.

The result-reader timing probes use accepted clean baseline `target/benchmarks/q8-final-f465b6f-primary-baseline/baseline.json`, source revision `f4bd438aa66db95296644f68769fd0c904f792f7`, `local_modifications=true`, one warmup, and three measurement runs. Reports `target/benchmarks/a2-diagnostics-m8-01`, `target/benchmarks/a2-diagnostics-m8-hits`, and `target/benchmarks/a2-diagnostics-m8-dets` record ratios of `0.283x`, `0.713x`, and `0.843x`, respectively. A separate one-run allocation diagnostic at `target/benchmarks/a2-diagnostics-result-reader-allocations` records maximum Stab allocation bytes of 12,288 for 01, 13,544 for HITS, and 28,672 for DETS, with zero resident-memory delta in all three rows. These dirty-worktree reports are development diagnostics only and are not promotable evidence.

## A2 Discovery And Dry-Run Planning

Commit `854fd127` adds a backend-neutral `CompilationRequestFingerprint`, sampling resource estimates, a records-owned codec registry, and a runtime `CapabilitySet`. The request identity includes model identity, operation, compiler schema, normalized compile options, and effective configurable limits. It intentionally excludes shots, seed, reference mode, codec, paths, and selected backend because those values either belong to execution or do not yet exist as caller choices.

Commit `6f1443aa` moves fixed-width output-size knowledge into `RecordFormat::estimate_output_bytes`. The sampling estimator delegates to that records owner instead of duplicating codec arithmetic. `01`, `b8`, and complete `ptb64` groups report exact checked sizes; value-dependent sparse encodings and incomplete PTB64 groups remain unknown.

Commit `b03b3c75` adds `stab capabilities`, `stab inspect`, and `stab plan sample`. Capability output is generated from the Clap command graph and product-owned gate, codec, compiler, parse-limit, and backend descriptors. Gate rows explicitly claim accepted circuit syntax rather than universal execution support, and the selectable-backend array remains empty until A4 creates a genuine caller-selectable backend.

`inspect` parses and fingerprints a circuit or DEM without compiling or executing it. `plan sample` compiles only for validation, then reports backend-neutral request identity, run configuration, and estimates without calling a sampling method. Successful `--format=json` output is one complete document on stdout; warning and failure `--error-format=json` remains JSON Lines on stderr. [Agent CLI schema version 1](../architecture/agent-cli-schema-v1.md) fixes this separation.

The first planning implementation reused the one-shot sampling path's expanded herald-column index list when correcting output width. Audit rejected that resource behavior because a compact repeat with a huge count could make a dry run expand billions of logical measurements. Planning now uses folded checked counting, while the actual sampler retains the index list it needs to filter emitted records. A ten-billion-iteration regression proves exact width without repeat expansion.

The benchmark design deliberately does not time `stab_cli::run_from` end to end. That would merge parsing, hashing, compilation, estimation, rendering, and I/O into an unactionable number and repeat the review-rejected mixed parse-and-estimate mistake. A2 instead uses four Stab-only product diagnostics with one measurement each: model fingerprint, inclusive request fingerprint, request estimate, and the sampler compile-and-release lifecycle. Existing circuit-parse evidence remains the parse measurement. These diagnostics have no Stim ratio, parity policy, waiver, self-regression baseline, legacy-manifest row, or formal completion claim.

Commit `dbdd4763` replaces expanded herald-column discovery in dry-run planning with folded checked counting and adds a ten-billion-repeat regression. It also gives sampler compilation a semantic correctness prerequisite that proves deterministic compiled plans preserve the circuit's execution contract instead of owning the API through a declaration-only test.

Commit `c2eb331c` added the first four executable Stab-only product diagnostics under runtime-group schema version 8. The diagnostic runner reused bounded subprocess supervision, isolated clean-source builds, host capture, calibration, immutable publication, and the `raw-work-v2` finish boundary, but it did not enter the paired Stim runner, parity policy, self-regression policy, release rollups, completion manifests, or the legacy M12 matrix.

The historical clean AArch64 PR-tier diagnostic checkpoint recorded:

| Group | 64 items | 4,096 items | 65,536 items | Report SHA-256 |
| --- | ---: | ---: | ---: | --- |
| Circuit model fingerprint | 48.170 ns/item | 46.470 ns/item | 46.554 ns/item | `c73a9b21af6aa0468466a88c4630cf0942153c66784858506c5bcf4ea6553b95` |
| Inclusive sampling-request fingerprint | 49.946 ns/item | 46.552 ns/item | 46.768 ns/item | `a9b29096d2e1ab0ec0ba98f0ba626439dc912c787f41723463cfab7e352946f2` |
| Sampling-request estimate | 6.116 ns/item | 5.519 ns/item | 5.481 ns/item | `00fb776844f696dd1e44b4eba7770b902cf20b88936151027fe6caef4ec9da36` |
| Sampler compilation as then implemented | 384.430 ns/item | 357.606 ns/item | 353.695 ns/item | `193e359c6989852a47bd3fe625bd98632332242a7dd8eee5f4c4030dbed328d9` |

All four reports bind clean revision `c2eb331c84b5040149d6d2597491ec748f9fe8cb`, correctness inventory `6512dfdc6056b4c03f95889a24ab311a9820845ae2615d7797146c1ca5dfcfcd`, and performance inventory `4a4cb7cb40753f42fa1f2c91a34e1a9bc5dcaf89c6696a5a71a0d461efb8a007`, with `local_modifications=false` before and after each run. The host was explicitly admitted as unverified, swap remained configured and unchanged, and the reports make no Stim parity, self-regression, release, or formal-completion claim. They are historical after the audit below and must not be cited as source-current diagnostics.

The immutable local report directories are `target/benchmarks/qualification/a2-circuit-model-fingerprint-c2eb331c`, `target/benchmarks/qualification/a2-sampling-request-fingerprint-c2eb331c`, `target/benchmarks/qualification/a2-sampling-request-estimate-c2eb331c`, and `target/benchmarks/qualification/a2-sampler-compile-c2eb331c`.

The focused milestone audit found no agent-command behavior defect but rejected four overstrong evidence claims. Commit `062f2cd5` freezes every successful schema-version-1 object shape, proves inspection accepts a circuit rejected by sampler compilation, executes every advertised codec through a nontrivial encode/decode fixture, covers checked estimator overflow directly, and narrows deterministic-plan wording so it does not pretend byte-identical reports are a shot-execution oracle.

The full benchmark review found that repeated sampler compilation replaced the previous plan inside the timed loop, so destruction was included despite a compile-only description. Retaining every plan until the finish clock would make memory scale with calibrated iterations, while per-iteration clocks would dominate the small workload. Commit `688495fd` therefore defines the scientifically honest operation as compile-and-release, validates a complete recompiled plan outside timing, and changes the measurement identity to `compile-and-release`. The same commit advances runtime-group schema to version 9 and enforces the previously declarative 600-second measurement-suite timeout through one outer monotonic deadline; every child timeout is the lesser of 30 seconds and remaining suite time.

The current correctness inventory contains 2,886 upstream cases, 2,567 public API items, and 1,801 evidence parents: 637 implemented, 17 evidence-close, and 1,147 planned. Its digest is `3c08ac35fe7379f427d5512f98033353844f25053a16093a1e0a61f8085cf976`; the current performance digest is `4902a52d00d291d6e2b8447c83262e9087bdc246de3ba3befc18ed1abcc09da8`.

Fresh schema-version-2 diagnostic reports bind clean revision `8b540bc2578dee432fe2c4213749796a6fdbdc5a`, runtime-group schema version 9, the current inventory digests above, and the enforced 600-second suite deadline:

| Group | 64 items | 4,096 items | 65,536 items | Report SHA-256 |
| --- | ---: | ---: | ---: | --- |
| Circuit model fingerprint | 47.839 ns/item | 47.374 ns/item | 46.704 ns/item | `3f0f6541d9a5e2edd8805e864d1291fba46cafc52c29daf0bb9b0573d6da4291` |
| Inclusive sampling-request fingerprint | 49.402 ns/item | 47.854 ns/item | 47.546 ns/item | `146be4d995d3a17930e970b2958269f0c48ab72952b1938a05fdee9070176225` |
| Sampling-request estimate | 6.051 ns/item | 5.469 ns/item | 5.509 ns/item | `026245dc43269fdee5c5a2dc19cf5a253b5a7125901718de2e1e3071d18c2ffe` |
| Sampler compile-and-release | 397.957 ns/item | 375.985 ns/item | 376.940 ns/item | `b20f42810e8f0690cdc34387d7c713e1df18f5b86ba68d8808e0cb4464eff1db` |

The immutable report directories are `target/benchmarks/qualification/a2-circuit-model-fingerprint-8b540bc2`, `target/benchmarks/qualification/a2-sampling-request-fingerprint-8b540bc2`, `target/benchmarks/qualification/a2-sampling-request-estimate-8b540bc2`, and `target/benchmarks/qualification/a2-sampler-compile-release-8b540bc2`. All four record `local_modifications=false` before and after. The AArch64 host remains unverified because swap-in counters increased during each run; swap configuration stayed enabled and unchanged. These results are development diagnostics, not Stim parity, Stab self-regression, release, or formal-completion evidence.

A2 remained active at checkpoint `8b540bc2` because exact circuit and DEM parser spans and the remaining operation-owned resource policies had not been implemented.

## A2 Parser And Resource Closure Candidate

The current worktree implements exact byte-oriented circuit and DEM diagnostics, source-order parsing, non-lossy opaque tag storage, byte serialization, and transform and analyzer preservation. Comments remain non-semantic and are discarded, but opaque comment bytes are admitted without changing the location or precedence of a later error.

All seven A2 policies are now concrete: `ParseLimits`, `CircuitFlattenLimits`, `DemFlattenLimits`, `DetectionConversionLimits`, `DemSamplerLimits`, `LogicalErrorSearchLimits`, and `SatMaterializationLimits`. Fixed semantic, representation, recursive-safety, and platform invariants remain non-overridable.

Independent review found and drove repairs for adjacent-operation fusion after byte parsing, lossy tag propagation through model-producing transforms, analyzer tag merging, rejected-line over-copying, quadratic opaque-range classification, caller-raised vector-capacity overflow, zero-width materialization undercounting, and recursive folded-DEM construction, transformation, and destruction. The folded DEM representation now constructs, clones, compares, formats, rounds, strips tags from, and drains deep programmatic trees iteratively while parser-owned recursive consumers retain their 256-level admission envelope.

The full workspace suite exposed one additional policy ambiguity after those reviews. DEM replay input is caller-owned storage but replay traversal is operation-owned work, so materialized and streaming replay now share a separate typed `ReplayWorkUnits` budget. Returned output retains its independent materialized-unit budget, while the historical combined replay-work and active-byte rejection boundaries remain unchanged. `sample_dem` validates the command-wide replay request before reading replay prefixes or activating outputs.

Review also found pre-admission recursion and output-lifecycle defects. Detection conversion now iteratively validates the fixed 256-level repeat envelope before direct or detector-frame recursive planning; exact depth 256 is accepted, depth 257 is typed, and 10,000-level programmatic inputs reject on a 64 KiB stack. `m2d` still opens every explicit path before converter setup for identity and open-error precedence, but does not activate or truncate primary and observable outputs until converter admission succeeds. `analyze_errors` applies the same open-before-parse and activate-after-analysis split, preserves a pre-existing output on rejection, and emits Stim's terminal newline for an empty model.

The pinned oracle now accepts reviewable `.hex` fixture payloads for arbitrary bytes. Live Stim v1.16.0 and Stab cases cover non-UTF-8 circuit comments and tags, opaque comments before a later syntax rejection, opaque DEM tags, and exact opaque analyzer output bytes. The exact Stab selectors continue to own byte spans, accessors, serializers, and transform propagation.

The regenerated correctness inventory contains 2,886 upstream cases, 2,877 public API items, and 1,893 evidence parents: 734 implemented, 17 evidence-close, and 1,142 planned. Its digest is `ccbeb26a1f4d10fedf68ef0aa66634c6b2b6607af76184598282501419c74a1d`. The regenerated performance inventory contains 127 checklist rows, 2,877 public API items, 173 groups, and 161 inherited manifest decisions. Its digest is `0d1fb8a08702dbb57b55e734e4735b3ce39f41388846d7b9ed715031feb88f54`.

An earlier dirty-worktree checkpoint passed workspace formatting, warnings-denied Clippy, all workspace tests, `just architecture::check`, correctness checking and regeneration, performance regeneration, and generated-status checking.

Review repairs have changed the source since that checkpoint, so those commands are historical development feedback and must be rerun.

This is not yet an A2 completion checkpoint because the changes remain uncommitted, final milestone and code reviews are in progress, and source-current clean-revision parser, allocation, and four-group diagnostic reports have not been produced.

The documentation audit converted the policy boundary rule into a per-policy and per-dimension evidence matrix in [the A2 resource policy inventory](../architecture/a2-resource-policy-inventory.md).

The matrix now executes every practical production maximum directly: both parser dimensions, both DEM-flatten repeat dimensions, detection record width and all three detection traversal dimensions, both logical-search repeat dimensions, and the 4,096-detector hyperedge degree.

The remaining dimensions use the documented reduced-boundary rule because their production ceilings would retain more than the ordinary single-test budget or execute tens of millions of work units. Each row names exact custom acceptance and first rejection, checked arithmetic or a dominating platform-capacity guard, and a concrete substitution rationale.

The audit additionally found that representational maxima were unsafe defaults for compact repeated detection input. Detection conversion now has a fixed repeat-depth envelope plus finite aggregate traversal, compiled-term, and compiled-byte defaults, performs dry admission before materialization, reuses one admitted plan during sampling, and avoids the CLI's previous duplicate compilation. The resolved specification gap is recorded in [milestone-spec-gaps.md](milestone-spec-gaps.md).

The architecture plan now contains the finite opaque-tag transform matrix and exact selectors for circuit flattening, noise removal, simplification, decomposition, unitary and QEC inversion, feedback inlining, DEM rounding and flattening, and flat and folded circuit-to-DEM analysis.

Comments, unlisted transforms, lossy display, and deferred ErrorMatcher provenance are explicitly outside that matrix.

The architecture plan also separates executable allocation correctness gates from timing reports.

Rejected parser suffix and rejected circuit-flatten payload allocation invariants are direct `cargo test` gates, the existing `m4-circuit-parse` compare supplies the Stim-relative timing and allocation observations, and the four A2 product diagnostics remain independent Stab-only phase timings.

A3 has not started. In particular, `stab-bits` and `stab-records` have not yet been extracted as physical crates.

## A2 Clean Closure

A2 closed against clean source revision `7b6c592b08f6a24d31a0673588dce7525b1c02c9` after focused implementation, CLI, oracle, benchmark-contract, and documentation commits.

| Commit | Purpose |
| --- | --- |
| `a74aab7a` | Add stable diagnostics, exact parser spans, fingerprints, capabilities, estimates, and operation-owned resource policies. |
| `3f910cb4` | Expose additive JSON diagnostics, capabilities, inspection, and dry-run sampling plans through the CLI without changing human defaults. |
| `adebbf35` | Qualify parser bytes, opaque metadata, resource boundaries, transforms, and CLI propagation against direct tests and pinned Stim cases. |
| `5ec0edfb` | Bind the four A2 product diagnostics and their correctness prerequisites to the current benchmark contracts. |
| `28bc3ce0` | Define the final A2 closure contract, resource-policy matrix, and executable evidence commands. |
| `7b6c592b` | Prevent allocation instrumentation from being mislabeled as Stim-relative timing evidence. |

The final milestone audit and full code review found no remaining source-current A2 blocker. Rust files remained below the 1,200-line project limit, all per-dimension resource-policy rows had direct selectors, generated correctness and performance identities matched their checked sources, and no A2 diagnostic was admitted to parity, self-regression, release, or formal-completion policy.

The parser allocation correctness gates passed:

- `parse_preallocation_is_bounded_by_the_admitted_line_prefix`
- `byte_parse_admission_does_not_copy_an_unterminated_rejected_line`
- `policy_preserves_defaults_and_rejects_before_output_allocation`

The clean parser baseline is `target/benchmarks/a2-circuit-parse-baseline-7b6c592b/baseline.json`, with SHA-256 `e3a0516c2a98ae4756a2683c2b55b62dee1b3f6e21fd85623ef06831cfb8db23`.

The clean timing report is `target/benchmarks/a2-circuit-parse-timing-7b6c592b/compare.json`, with SHA-256 `7126fd54e2470bd8055cd9b63665565cae227ffb3e1d03d797d97d7aeff97db`. It records `local_modifications=false`, a dense parse ratio of `1.135x`, a sparse parse ratio and headline ratio of `1.200x`, and a passing unchanged `1.25x` beta gate.

The clean allocation observation is `target/benchmarks/a2-circuit-parse-allocations-7b6c592b/compare.json`, with SHA-256 `3e19749290b3a91c0224cc14caecffe6704666ffb907e47ec1d9af5c6474ba99`. It records `local_modifications=false`, a dense accepted-workload allocation peak of 1,152 bytes, a sparse allocation peak of 288,000 bytes, and `not-evaluated-instrumented` for both timing pass/fail and the beta gate. Its instrumented wall-time ratios are context only and make no parity claim.

The earlier `28bc3ce0` allocation artifact is preserved but review-rejected because it incorrectly applied the timing beta gate to allocator-instrumented wall time. The dirty `target/benchmarks/a2-allocation-contract-probe-dirty` report is also non-promotable. Neither path may be reused as current evidence.

The four clean product-diagnostic reports are:

| Group | Report | SHA-256 | Small / medium / large median time per item |
| --- | --- | --- | --- |
| Circuit model fingerprint | `target/benchmarks/qualification/a2-circuit-model-fingerprint-7b6c592b/report.json` | `0e86cc4a7bf3a909fc08f8c37938785033418b23bf00ee9fd14c7fcd4c57a65f` | 46.192 ns / 45.363 ns / 45.507 ns |
| Sampling request fingerprint, inclusive | `target/benchmarks/qualification/a2-sampling-request-fingerprint-7b6c592b/report.json` | `f9ebc69e71ac9531e11962a7e1b1d52c59bd7ca12a976acf5ae596127041bb33` | 47.802 ns / 45.405 ns / 45.532 ns |
| Sampling request estimate | `target/benchmarks/qualification/a2-sampling-request-estimate-7b6c592b/report.json` | `bffb6d20bcf272d752eb89b47aba220bbbf5184b1fe1e190461aa20b6272a332` | 6.147 ns / 5.387 ns / 5.589 ns |
| Sampler compile and release | `target/benchmarks/qualification/a2-sampler-compile-release-7b6c592b/report.json` | `55ba766fb013832e5a5c500be27a32046f1d977dd52248c58f35b901c9c2f34b` | 396.221 ns / 371.282 ns / 372.552 ns |

Every diagnostic binds the clean source revision before and after execution, the frozen correctness digest `ccbeb26a1f4d10fedf68ef0aa66634c6b2b6607af76184598282501419c74a1d`, the frozen performance digest `0d1fb8a08702dbb57b55e734e4735b3ce39f41388846d7b9ed715031feb88f54`, all three source-owned scales, a complete semantic witness, and `raw-work-v2`. The reports are explicitly `product-diagnostic` with no parity or regression result. The host is unverified because swap-in counters changed during each run, so the reports remain development diagnostics and cannot support release evidence.

The source revision passed formatting, warnings-denied workspace Clippy, all workspace tests, warnings-denied rustdoc, architecture enforcement, implemented oracle fixtures, the live 62-case result-format oracle, correctness and performance check/regeneration, generated-status checking, benchmark smoke, and staged pre-commit validation.

A2 does not claim physical modularity. At its closure revision, `stab-bits` and `stab-records` still do not exist as Cargo packages; that extraction is the first A3 task.
