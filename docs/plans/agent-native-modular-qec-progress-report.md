# Agent-Native Modular QEC Progress Report

Current as of 2026-07-28.

## Status

- A0 architecture contract and baseline: complete.
- A1 logical ownership and dependency enforcement: complete.
- A2 diagnostics, resources, fingerprints, and capabilities: complete at clean source revision `7b6c592b08f6a24d31a0673588dce7525b1c02c9`.
- A3 stable packed records and codecs: complete; product timing and allocation evidence binds clean revision `cb0f2ddbb19a99e16f27471b91966312a4404f79`, and the final oracle ownership repair is commit `07df4b33`.
- A4 sampling compiler, plan, session, and sink: complete at clean source revision `af71182ea60146986c4b4aac9d5713484eb7e449`.
- A5 detection and DEM batch pipelines: complete at clean source revision `b8e3f459d2a8817aa98ca0d71072a9529fa9fe9c`.
- A6 physical component extraction and Nightly isolation: active.
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

At that A2 audit checkpoint, A3 had not started. In particular, `stab-bits` and `stab-records` had not yet been extracted as physical crates.

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

## A3 Stable Bits Extraction

The first half of A3 completed at clean source revision `3de29da0c177c150f74b1fa93ed5217db186ead1`.

`stab-bits` is now a physical Cargo package and a Stable Rust 1.97.1 leaf dependency of `stab-core`. It owns checked packed bit storage, borrowed views, scalar word kernels, sparse XOR storage, matrix storage, and transpose behavior. Quantum-specific Clifford SIMD and Pauli-word semantics remain in `stab-core` until the later algebra and SIMD-kernel extractions, so the leaf package does not acquire quantum semantics or a Nightly requirement.

The extraction introduced `BitWordsMut`, a guarded mutable word view that restores the unused-tail-bit invariant on drop. Existing `stab_core::bits` and root paths remain compatibility re-exports while `stab_bits::*` is the canonical component API. Qualification inventory generation now resolves public re-exports from external workspace crates against their canonical leaf inventories and fails closed when a canonical inventory is unavailable.

The current correctness inventory has digest `7cab7ce523970408fdbcc437c190aede4ed16ba7921a33eb5e17bb2fbc455691`, 2,886 upstream cases, 3,041 exported API items, and 1,894 evidence cases. The current performance inventory has digest `e01aec62e8ce2b5820a5dc1178a96d882f403541991369c22a7bf54e5ee9ba30`, 127 checklist rows, 3,041 exported API items, 173 groups, and 161 inherited rows. No behavioral `stab_bits::*` item remains assigned only to planned correctness evidence.

The clean pre-extraction M5 baseline is `target/benchmarks/a3-pre-extraction-baseline-6d10e8f8/baseline.json` with SHA-256 `0f23df41ed5afdda9a00312acfa55ff50a475de11966e18f6e91106bbf06d7d1`. The matching pre-extraction compare is `target/benchmarks/a3-pre-extraction-compare-6d10e8f8/compare.json` with SHA-256 `0d64aca154e839326c3410db862e7c3aaa8d4a9b84e0f155697001b95668c13f`.

The clean post-extraction reports are:

| Scope | Report SHA-256 | Pre | Post | Interpretation |
| --- | --- | --- | --- | --- |
| Generic XOR | `41815c885e41750145c04bfa415ed7a2b38f86ea67d12adad61adfe79e438a2c` | 17 ns | 16 ns | No regression |
| Not-zero probe | `41815c885e41750145c04bfa415ed7a2b38f86ea67d12adad61adfe79e438a2c` | 1 ns | 4 ns | Sub-single-digit-nanosecond timer noise on mismatched Stim and Stab work sizes; not a material regression claim |
| Sparse row XOR | `127cf72804994ea35a8d231e12bc5170598d320a485d0d73e69f327d478d7665` | 16.096 us | 15.440 us | Improved |
| Sparse item XOR | `127cf72804994ea35a8d231e12bc5170598d320a485d0d73e69f327d478d7665` | 13 ns | 13 ns | Unchanged |
| Matrix row XOR | `da1e688fe6b22a82bb115dc33f975527ebfb84743b1d58ce4f33cbf224ff9d7c` | 416 ns | 416 ns | Unchanged |
| Matrix transpose | `da1e688fe6b22a82bb115dc33f975527ebfb84743b1d58ce4f33cbf224ff9d7c` | 592 ns | 592 ns | Unchanged |

Stable package checks, warnings-denied workspace Clippy, complete workspace tests, warnings-denied `stab-bits` rustdoc, architecture enforcement, correctness and performance inventory regeneration, generated-status checking, the complete implemented oracle fixture run, and staged pre-commit validation passed at the extraction revision.

A3 was not complete at the bits-only checkpoint. The records extraction below addresses the remaining implementation work; clean post-commit evidence and closure audits still determine milestone completion.

## A3 Stable Records Extraction

`stab-records` is now a physical Stable Rust 1.97.1 package with the sole product dependency `stab-records -> stab-bits`. `stab-core` depends on both leaf crates and retains compatibility re-exports and lossless conversion from records diagnostics into `CircuitError`. The shared compatibility corpus moved from `ops/` to `test-support/compat-corpus`; product crates may use it only as a development dependency, so qualification plumbing is not a runtime product dependency.

The component owns strict `01`, `b8`, `r8`, HITS, DETS, and PTB64 codecs; structured format diagnostics; typed `DetsLayout`; shot-major and at-most-64-shot bit-plane batches; distinct measurement, detector, observable, sampled-error, and correction widths; generic first-error-preserving visitors; and typed measurement, detection, and DEM-sample sink traits with in-memory codec implementations. Detector and observable planes remain separate until explicit encoding. PTB64 sinks reuse one 64-shot buffer, reject incomplete final groups recoverably, and preserve zero-shot, nonzero-width behavior.

The records package runs all 62 checked corpus cases directly. Focused tests cover typed namespace semantics, exact bytes for all six formats, layout transpose round trips across zero through 64 shots, width mismatch rejection, visitor cancellation, and allocation growth independent of record count. A workspace-wide pass caught and repaired one facade issue before commit: `CircuitError` now converts both the facade and canonical records `FormatError`, preserving existing `?`-based callers of re-exported constructors.

Qualification rustdoc identity now resolves generic trait arguments through fully qualified rustdoc paths. This is necessary because `stab_core::FormatError` and `stab_records::FormatError` legitimately have the same terminal type name but distinct `From` implementations. Compatibility re-exports now share the canonical records evidence owner instead of creating hundreds of duplicate planned facade owners. The current correctness inventory has digest `d85172a83661b35543c647d0fdf6b3e8752cb5024fc0ade480b8245709ec59a8`, 2,886 upstream cases, 4,009 exported API items, and 1,917 evidence cases. The current performance inventory has digest `d97eabbde9e260f1cc4a2fa3a97b24e36ae9ec573175ea02d6fc51b5bde929a0`, 127 checklist rows, 4,009 exported API items, 176 groups, and 164 inherited rows. No behavioral `stab_records::*` item remains assigned only to planned correctness evidence.

Three source-owned report-only component rows were added without inventing a Stim ratio:

- `m8-record-writer-contract` measures typed B8 shot-major writing and PTB64 bit-plane writing.
- `m8-record-batch-transpose-contract` measures both public shot-major and bit-plane conversions.
- `m8-record-dets-layout-contract` measures typed detector and observable DETS parsing.

The initial dirty timing diagnostic is `target/benchmarks/a3-records-dirty-compare`. It records 490 million bits per second for typed B8 output, 31.08 billion bits per second for direct PTB64 bit-plane output, and 29.81 and 27.08 billion bits per second for the two transpose directions. Its DETS row incorrectly derived “bits” from namespace width even though the parser consumes nine tokens per record; that historical rate is review-rejected and must not be cited. The source-owned runner now reports DETS records per second. The post-reservation allocation diagnostic is `target/benchmarks/a3-records-dirty-allocations-v2`; both writers and both transpose directions perform exactly one measured allocation, while the DETS parser remains at four allocations and 464 total allocated bytes for 4,096 records. Both reports identify source revision `a848775937abe65f1d6270a9738600cccb9788fc` with `local_modifications=true`, so they are development diagnostics only and make no promotable regression or Stim-parity claim.

`SampleFormat` remains the five-format legacy writer enum, while `RecordFormat` is the six-format component registry that includes PTB64. The overlap is recorded migration debt, not an equivalence claim. Specialized `for_each_*` functions remain convenience adapters; generic `try_for_each_*` functions are the modular error-preserving visitor boundary.

The A3 audit found two under-specified resource phrases. Returning a generic visitor error is now the defined cancellation signal and preserves the first error without delivering another record. Dense and packed HITS/DETS readers now consume strict lexer events directly, so their allocation is independent of duplicate-token count; a 16,384-duplicate regression proves this for both representations. Raw sparse and typed-token visitors still retain one encoded record because duplicate order is their semantic result. In-memory codec sinks may retain caller-requested encoded output bytes, while all additional scratch remains width- and batch-bounded. `MeasureRecordWriter::begin_dets_result_type` gives component code a typed namespace selector; the raw byte selector remains an explicitly documented compatibility adapter.

The final pre-commit milestone audit and full-code review found no P0/P1 compatibility, resource-safety, benchmark-semantics, or crate-boundary defect. Their only code-quality finding was that three touched qualification modules sat just above the 1,200-line project threshold. Simulator classification, stable case-ID generation, and evidence-only export policy now have separate owned modules; the three parent files are 1,082, 1,197, and 1,197 lines. The extraction is committed at `46abdac2`, the record benchmark contracts are committed at `b8dff63c`, and the synchronized pre-evidence documentation checkpoint is committed at `cb0f2ddb`.

## A3 Clean Closure Evidence

The accepted A3 component and compatibility compare reports below bind clean product revision `cb0f2ddbb19a99e16f27471b91966312a4404f79` with `local_modifications=false`. The later oracle repair changes fixture dispatch only, not product or benchmark code. The two baseline artifacts bind pinned Stim v1.16.0 rather than a Stab revision. The final row is the separately identified clean pre-extraction comparison at revision `a848775937abe65f1d6270a9738600cccb9788fc`.

| Evidence | Artifact | SHA-256 |
| --- | --- | --- |
| Direct component baseline | `target/benchmarks/a3-records-clean-baseline-cb0f2ddb/baseline.json` | `fc41686bf931b0181118f52731be3333a58f94991b791d2aed6713d88ef36691` |
| Direct component timing | `target/benchmarks/a3-records-clean-compare-cb0f2ddb/compare.json` | `78849d336e8f7e65570fff4a376810987f6a1133d73b1bcf2953ccd7abd8b55f` |
| Direct component allocation | `target/benchmarks/a3-records-clean-allocations-cb0f2ddb/compare.json` | `1157ba5a9a92000cfa330255f216b11fe578b7b6bb41ecd09cb8b411db011118` |
| Compatibility baseline | `target/benchmarks/a3-records-compat-baseline-cb0f2ddb/baseline.json` | `cbe09e3e5176893b4fe243b138c46daf171f87646c88279bfea19c1796223732` |
| Source-current compatibility timing | `target/benchmarks/a3-records-compat-compare-cb0f2ddb/compare.json` | `3698a24228c9829ba61dc5deacc7c47152a7a3a1002df5ba6439091460f38737` |
| Clean pre-extraction comparison | `target/benchmarks/a3-records-pre-extraction-compat-compare-a8487759-retry3/compare.json` | `649b1c344aa546c6d2cad32146d2ab262c5e6425a79f5ee1c217438a6ecd7906` |

The direct component report uses one warmup and three recorded runs. Its source-owned report-only measurements are:

| Measurement | Median observation |
| --- | --- |
| Typed B8 shot-major writer | 468.3 million bits per second |
| Direct PTB64 bit-plane writer | 23.09 billion bits per second |
| Shot-major to bit-plane transpose | 30.10 billion bits per second |
| Bit-plane to shot-major transpose | 27.30 billion bits per second |
| Typed detector and observable DETS parser | 12.67 million records per second |

No Stim ratio is claimed for these component-only workloads. Allocation instrumentation records one measured allocation for every workload: 80,000 bytes for each writer and shot-major-to-plane conversion, 80,384 bytes for plane-to-shot conversion, and 16 bytes for the packed 4,096-record DETS benchmark. The dedicated dense and packed resource tests separately prove that accepted parsing allocation does not grow with record count or duplicate-token count.

The source-current compatibility report also uses one warmup and three recorded runs. All eight Stim-comparable rows pass the unchanged `1.25x` gate:

| Row | Stab over Stim |
| --- | ---: |
| `m7-convert-01-to-b8` | `0.273x` |
| `m7-convert-b8-to-01` | `0.300x` |
| `m7-convert-dets-to-b8` | `0.175x` |
| `m7-convert-ptb64-to-01` | `0.269x` |
| `m8-measure-reader-01` | `0.331x` |
| `m8-measure-reader-b8` | `0.959x` |
| `m8-measure-reader-hits` | `0.569x` |
| `m8-measure-reader-dets` | `0.752x` |

`m7-convert-01-to-ptb64` and `m8-measure-reader-ptb64-contract` remain contract-only because pinned Stim v1.16.0 has no faithful comparator for those exact operations. They are measured but correctly report `not-proven`.

The clean pre-extraction report binds revision `a848775937abe65f1d6270a9738600cccb9788fc` with `local_modifications=false` and reuses the identical compatibility baseline and workload identifiers. Of 18 matched Stab measurements, 14 are faster after extraction and four are slower. The largest observed improvement is 29.6%, and the largest observed slowdown is 10.5%; the latter is below the source-owned 15% self-regression boundary. This separately scheduled three-run comparison is diagnostic rather than an alternating paired self-regression experiment, so the report supports the absence of a material extraction regression but does not establish a formal regression baseline.

The first interrupted pre-extraction path produced no report. The `retry1` path was rejected before measurement because its detached worktree had not materialized the pinned Stim submodule. The `retry2` invocation measured the rows but rejected an absolute publication path. None is reused as evidence; `retry3` is the sole accepted pre-extraction report.

The final closure run of `just oracle::run --implemented-only` caught one integration omission that narrower checks had missed: six result-format coverage rows still dispatched `cargo test` to the now-empty `stab-core` `result_formats` filter. Commit `07df4b33` retargets all six rows to the canonical `stab-records` package and adds a focused manifest-ownership regression. The complete implemented-only oracle then passed. This was repaired as an implementation defect rather than logged as a specification gap.

The final independent milestone audit and full-code-review found no P0, P1, or P2 A3 issue. They identified two P3 documentation precision problems, now corrected: baseline files bind pinned Stim rather than a Stab commit, and the packed DETS allocation artifact must not be cited as the sole proof of the broader dense-allocation invariant.

A3 is complete. Stable users can consume `stab-bits` and `stab-records` without `stab-core` or Nightly, exact Stim result bytes remain unchanged, and the facade retains compatibility adapters without owning a second implementation. Formal post-refactor correctness and performance qualification remains a later program-level task rather than an A3 claim.

## A4 Sampling Plan, Session, And Sink

A4 completed at clean source revision `af71182ea60146986c4b4aac9d5713484eb7e449`.

| Commit | Purpose |
| --- | --- |
| `f984f577` | Add reusable typed bit-plane sampling batches to `stab-records`. |
| `37063c68` | Introduce the sampling compiler, immutable plans, mutable sessions, typed sinks, cancellation, poisoning, and backend-bearing plan fingerprints. |
| `043408a6` | Route `stab sample` through the public plan, session, and sink path while preserving the CLI contract. |
| `67f10314` | Separate sampling phases and make the four Stim-comparable rows process-symmetric. |
| `750fd6f7` | Assign direct correctness and performance ownership to the new sampling contracts. |
| `88d95a3f` | Define the sampling component boundary and its compatibility-adapter policy. |
| `af71182e` | Repair the process-comparison repetition schedule after final review found a three-Stim-versus-nine-Stab asymmetry. |

`SamplingCompiler` now lowers a typed request into an immutable, cloneable, `Send + Sync` `SamplingPlan`. A mutable non-`Sync` `SamplingSession` owns the RNG, reference state, private simulator frame, bounded reusable bit-plane batch, progress, cancellation, and poison state. Direct-Z, small-frame, and general-frame execution remain private plan variants. Scalar is the only registered backend in A4; an explicit portable-SIMD request fails before lowering instead of pretending that a second implementation exists.

The session constructor performs checked fallible reservation and rejects conservative reusable storage estimates above 256 MiB before allocation. Empty runs consume no randomness and do not touch the sink. Successful and cancelled runs finalize one sink lifecycle; sink or engine failures preserve the first error, report exact committed progress, stop immediately, and poison the session. Pre-execution validation and counter-overflow errors do not poison a reusable session. `CompiledSampler` remains a documented compatibility adapter until the A6 facade curation.

The public CLI path now uses the same compiler, plan, session, and typed records sink. Exact zero-shot no-I/O behavior, writer and flush error propagation, path preflight, reference-sample modes, seeded chunking, every private execution variant, frozen pre-A4 vectors, cancellation, poisoning, bounded post-warmup allocation, and wide HITS, DETS, and PTB64 output have direct regression coverage.

The regenerated correctness inventory contains 2,886 upstream cases, 4,501 exported API items, and 1,957 evidence parents. Its digest is `091a03280f829e783d1c5acd7b1dbd5fb8bd37ccdea85bfcc0ddec9a9e8b863b`. The regenerated performance inventory contains 127 checklist rows, 4,501 exported API items, 176 groups, and 164 inherited rows. Its digest is `4aa88447f230845873bcc44657f037e48f2f2147c0260412d055efc2c221bc95`.

### A4 Clean Evidence

The accepted A4 reports bind clean revision `af71182ea60146986c4b4aac9d5713484eb7e449` with `local_modifications=false`.

| Evidence | Artifact | SHA-256 |
| --- | --- | --- |
| Phase-diagnostic baseline | `target/benchmarks/a4-sampling-diagnostic-baseline-af71182e/baseline.json` | `48514c304bd50b12cc1646621464e7ce33ff8ea031bfdc9d45da85dea467c23b` |
| Phase-diagnostic comparison | `target/benchmarks/a4-sampling-diagnostic-compare-af71182e/compare.json` | `f3df3db2a7e0ee6561bd0493a9e696c751b9be72a1aa701e396e9f150398909e` |
| Process-parity baseline | `target/benchmarks/a4-sampling-parity-baseline-af71182e-matched/baseline.json` | `ee78234fa38b7028b24ac6add2e22437b37e6ec88b62f4be3349caa08f7b472e` |
| Process-parity comparison | `target/benchmarks/a4-sampling-parity-compare-af71182e-matched/compare.json` | `478425eb8aa774c1662c9718a1328220c6fb670a5f08addfc61bfa3af23dea52` |

The report-only phase diagnostic records 2.155 million automatic or explicit-scalar compilations per second, 6.944 million session constructions per second, 444.4 million 64-shot witness-sink shots per second, 800.0 million prebuilt-batch consumptions per second, 181.8 million B8 encodes per second, 285.7 million shots per second across sixteen four-shot runs on one session, 447.6 million shots per second for the 1,024-shot in-process row, and 500.9 million shots per second for the one-million-shot row. These isolated workloads have no faithful Stim comparator and make no Stim-parity claim. Their new identities are unseeded candidates; the 15% Stab self-regression policy starts only with a later identity-matched controlled-host measurement.

The four process-symmetric rows use the same bounded supervisor, standard input, command arguments, iteration policy, and discarded standard output for Stim and Stab. An untimed Stab preflight checks a frozen pre-A4 output witness. All four pass the unchanged `1.25x` gate:

| Row | Stab over Stim |
| --- | ---: |
| Repetition contract | `1.025x` |
| Rotated surface-code contract | `1.012x` |
| Unrotated surface-code contract | `0.998x` |
| High-repeat contract | `1.005x` |

Each accepted process row records exactly three timed Stim launches and three timed Stab launches. Final review found that the earlier `88d95a3f` comparison instead nested the Stab runner's three launches inside three outer recorded runs, producing three Stim launches versus nine Stab launches despite an identical-iteration claim. That comparison is review-rejected and retained only as history. Commit `af71182e` makes one runner invocation own one process launch, lets the shared outer `--measurement-runs 3` policy own the median population, and strengthens report-only throughput witnesses with boundary output bytes.

The clean revision passed formatting, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture enforcement, the complete implemented oracle run, the live 62-case result-format oracle, correctness and performance check/regeneration, generated-status checking, benchmark smoke, and staged pre-commit validation. A dirty development probe and the asymmetric `88d95a3f` process comparison remain non-promotable and are not used for closure.

A4 is complete. Sampling execution imports no codec, filesystem, CLI, or ops API; the CLI does not bypass the plan/session/sink architecture; process-equivalent rows remain inside the unchanged Stim gate; and unlike phase identities are explicitly unseeded instead of being compared to a misleading historical operation.

## A5 Detection And DEM Batch Pipelines

A5 is complete at clean source revision `b8e3f459d2a8817aa98ca0d71072a9529fa9fe9c`.

`stab_core::execution` now owns three distinct public families:

- `MeasurementToDetectionCompiler` -> `MeasurementToDetectionPlan` -> `MeasurementToDetectionSession` -> `DetectionSink`
- `DetectionSamplingCompiler` -> `DetectionSamplingPlan` -> `DetectionSamplingSession` -> `DetectionSink`
- `DemSamplingCompiler` -> `DemSamplingPlan` -> `DemSamplingSession` or incremental `DemReplaySession` -> `DemSampleSink`

Plans are immutable and shareable. Sessions own reusable mutable conversion, reference, detector-frame, RNG, error-record, and bounded 64-shot batch state, plus exact progress, cancellation, and poisoning. Direct detector-frame and fused sample-convert implementations remain private detection variants. Detector-only sampling, sampled-error sampling, and replay remain distinct DEM algorithms. Finite-shot sampling materializers and visitors delegate through the new execution paths. `CompiledDetectionConverter` remains the public low-level per-record kernel used by conversion sessions, while unknown-length iterator DEM replay retains direct folded traversal; neither compatibility exception is used by the CLI.

The CLI now routes `detect`, `m2d`, and `sample_dem` through typed session sinks. `m2d` retains record-at-a-time initial delivery so a malformed later record preserves already committed output. `sample_dem` opens and retains only its inputs, validates replay work and the complete replay prefix, rewinds the retained replay handle, and only then creates, identity-checks, and activates output sinks. Detector, observable, and sampled-error planes remain separate until CLI encoding; PTB64 routing buffers exactly one complete 64-record group per output stream.

Focused correctness evidence covers materialized equivalence, sweep-conditioned conversion, adapter composition, one-sink incremental delivery, valid-prefix delivery, direct-versus-fused detection, seeded partitioning, cancellation including the replay finish boundary, sink failure progress, poisoning, DEM families, incremental replay without RNG advance, replay abandonment, exact malformed-prefix progress, caller active-byte admission including sampled-error compatibility scratch, aggregate fused-session admission before component construction, complete direct-frame retained-plan admission, zero-shot behavior, and record-count-independent allocation. A 4,096-record matrix crosses multiple batches for every supported primary and side-output format of `detect`, `m2d`, and `sample_dem`; malformed text, packed, and PTB64 replay prefixes prove all three output roles remain absent or retain sentinel bytes, and a delayed hardlink substitution proves retained input identities close the path-check race. The regenerated correctness inventory contains 2,886 upstream cases, 4,761 exported API items, and 1,992 evidence parents with digest `858cc523daed7ad6eb99468168713e19146b1201b86c40a08aa27db18ffc400e`.

The regenerated performance inventory contains 127 checklist rows, 4,761 exported API items, 179 groups, and 167 inherited rows with digest `f2b78adf7f2bdbb94d2a8c19a45a23ec3485c2768b166a824732f9e64d2fa680`. A5 adds no formal runtime group. Three non-primary report-only legacy-manifest rows expose source-owned phases and remain future candidates in the comprehensive inventory, preserving the finite release matrix.

Dirty local diagnostics, retained only as development evidence, observed:

| Phase | Median observation |
| --- | ---: |
| Detection-plan compile-and-release | 0.688 us |
| Detection session sample-to-detection | 56.19 million shots/s |
| Detect PTB64 routing | 20.09 million shots/s |
| Measurement-to-detection plan compile-and-release | 0.816 us |
| Measurement-to-detection bounded batch conversion | 55.56 million shots/s |
| DEM-plan compile-and-release | 4.816 us |
| DEM detector-only session | 2.861 million shots/s |
| DEM sampled-error session | 2.380 million shots/s |
| DEM replay session | 3.382 million shots/s |
| Sample-dem PTB64 routing | 19.76 million shots/s |

The combined dirty phase artifact is `target/benchmarks/a5-phase-compare-20260728`. It binds local modifications and makes no Stim parity or Stab self-regression claim. The new phase identities remain report-only and unseeded.

The dirty process-symmetric probe at `target/benchmarks/a5-cli-contract-compare-20260728` observed `detect` at `0.975x`, `m2d` at `0.977x`, and `sample_dem` at `0.983x` pinned Stim. Each row contains exactly one Stab process measurement and one Stim process measurement, uses the same command shape and launch count, and validates the complete Stab output outside timing. The eleven process-equivalent rows are `m9-detect-text-cli`, `m9-detect-bitpacked-cli`, `m9-detect-primary-matrix-contract`, `m9-m2d-text-cli`, `m9-m2d-bitpacked-contract`, `m9-m2d-primary-matrix-contract`, `m11-sample-dem-cli`, `m11-sample-dem-sparse-contract`, `m11-sample-dem-dense-contract`, `m11-sample-dem-repeated-contract`, and `m11-sample-dem-high-detector-contract`. Allocation-enabled runs keep process timing but use an untimed in-process mirror with the same command and witness for product-semantic allocation and retained-memory fields. Commit `56141a32` reseeded the former core-proxy memory identities from clean revision `06318e49`; A5 closure verifies those replacement identities with isolated warmed rows. The earlier local-modification results remain diagnostic only.

The first full-code-review of clean commit `56141a32` found five additional defects. Incremental measurement conversion accepted arbitrary sinks across batches and could not report finish-failure prefix progress; direct detector-frame compilation omitted its retained executable circuit from `max_compiled_bytes`; the materialized DEM replay convenience API scanned caller records before poison and work admission; all eleven process rows validated only Stab output; and the report-only A5 phases calculated but did not enforce semantic witnesses. The repaired implementation binds one sink to one delivery, rejects repeated or post-finish operations, poisons abandoned committed delivery, dry-runs complete direct-frame retained storage before fallible materialization, admits replay work before width traversal, and preflights independent frozen Stim and Stab witnesses. New exact qualification parents own each lifecycle and resource boundary.

The follow-up milestone audit and full-code-review of clean commit `1936f9d3` found two remaining proof defects plus documentation drift. Direct-frame aggregate admission still materialized conversion terms before checking the combined limit, the executable storage scan constructed temporary filtered `XCZ` and `YCZ` instructions, and the exact owner did not exercise compact repeats or mixed filtered targets. Stateful report-only phases froze only their first result and generated later expected values with the same Stab implementation, while compile-and-release phases had no exact plan witness. Commits `0bdff4a5`, `18d899dd`, `e67ec8a0`, and `aa5ad290` repair these findings: conversion admission is dry until aggregate storage passes; filtered executable bytes are counted without temporary instructions; the exact owner checks compact repeats, filtered targets, exact acceptance, first excess, and rejection-path allocation; observable IDs avoid per-term formatting allocation across the exact f64/u64 boundary; compile phases check source-owned plan dimensions; and stateful output phases check frozen ordered SHA-256 witness sequences. The milestone contract now distinguishes compile fingerprints from output shot-count and sequence witnesses instead of treating one vague “digest witness” as sufficient for every phase.

The independent milestone audit of clean commit `31692bfb` found no remaining implementation defect or specification loophole. The separate full-code-review found two benchmark-boundary defects: allocation-enabled phase rows ran one extra operation through the same closure, extending fixed witness vectors and invalidating their digests, while all phase timers stopped only after plan checks, witness extraction, shot validation, and sequence bookkeeping. The benchmark harness now samples the finish clock immediately after raw product work, performs semantic validation afterward, and gives allocation observation independently initialized sessions and sinks. A fake-clock ordering test proves the finish-clock boundary, and an allocation-enabled three-row test plus a production probe proves the extra memory operation cannot alter timed witness sequences.

All baseline, comparison, beta-gate, phase, and memory artifacts produced through `31692bfb` predate the final timing-boundary repair and are retained as historical diagnostics only. Failed witness-discovery paths under `target/benchmarks/a5-stim-witness-probe`, `target/benchmarks/a5-phase-witness-probe`, the later `a5-sequence-probe-*` paths, and the review-rejected allocation-enabled phase paths are likewise immutable development history.

The final audit of clean commit `b8e3f459` confirmed that plan dimensions are now checked only in untimed preflight, timed compilation contains only compilation, optimizer opacity, and release, PTB64 digest writers cross the finish-clock boundary before witness extraction, and memory operations use independent state. The final full-code-review found no P0, P1, or P2 defect in A5 product behavior, resource admission, lifecycle semantics, benchmark validity, or the repaired process-supervisor test. Neither review found an A5 specification loophole.

The source-current phase report is `target/benchmarks/a5-clean-phases-b8e3f459`, with compare hash `859ef2148766303710a5237c77a14224cb6a517eeece950490f835485cab6253` and report hash `6e8fe83bd7a9b54ce1c08988fd035f18dae2005398861289edbd9cdf4759e56a`. It records:

| Phase | Median |
| --- | ---: |
| Detection plan compile-and-release | 0.736 us |
| Detection session sample-to-detection | 53.60 million shots/s |
| Detect PTB64 routing | 20.27 million shots/s |
| Measurement-to-detection plan compile-and-release | 0.832 us |
| Measurement-to-detection batch conversion | 54.79 million shots/s |
| DEM plan compile-and-release | 5.088 us |
| DEM detector-only session | 2.934 million shots/s |
| DEM sampled-error session | 2.507 million shots/s |
| DEM replay session | 3.364 million shots/s |
| Sample-dem PTB64 routing | 19.92 million shots/s |

The source-current process comparison is `target/benchmarks/a5-clean-cli-beta-b8e3f459`, with compare hash `71d9a889a1fb36844c63015e0bffd1d1e924c82e82a9c788a2cbcf4dd8958eb6` and report hash `657dbf5c77e0475fa015ca7a9aba3f2371d4d3fbc4b8c8b18c22a691fdfe2dc6`. All eleven rows pass the unchanged `1.25x` Stim gate. Ratios range from `0.999066x` for text detection through `1.072982x` for sparse DEM sampling; no waiver or threshold change was introduced.

The first isolated source-current memory attempts under `target/benchmarks/a5-clean-cli-memory-b8e3f459-*` intentionally remain failed evidence because they omitted the benchmark's declared `--warmup`; ten rows therefore charged 68–86 KiB of initialization RSS against a 64 KiB page-noise allowance. Observer and extra-product-warmup experiments under the `a5-memory-*-probe-20260728-*` paths were falsified and removed from source. The clean three-run candidates under `target/benchmarks/a5-memory-isolated-baseline-candidate-b8e3f459-*` showed that matching the source-owned warmup restores the established measurement identity without changing its baseline. Final gated reports under `target/benchmarks/a5-clean-cli-memory-b8e3f459-warm-*` all pass, with exact product allocation peaks of 120,859–123,204 bytes and resident deltas of 0–8 KiB against unchanged allowances.

Every final phase, timing, and memory report records commit `b8e3f459d2a8817aa98ca0d71072a9529fa9fe9c` with `local_modifications=false`. Formatting, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture enforcement, API docs, Stim version validation, the live 62-case result-format corpus, the complete implemented oracle run, correctness and performance check/regeneration, generated-status checking, and benchmark smoke all passed. A5 is closed; A6 owns the remaining physical crate extraction.

## A6 Physical Component Extraction

A6 is active.

The exact source, API, feature, test, and benchmark destinations were frozen first in [the A6 physical component extraction map](../architecture/a6-component-extraction-map.md).

The first physical slice extracts `stab-algebra` as a Stable Rust 1.97.1 package over `stab-bits`. Pauli, Clifford, tableau, flow, conversion, algebra-error, algebra-resource, and scalar quantum-word implementations now have one canonical owner. `stab_core::stabilizers` contains only compatibility reexports.

The extraction removes the previous direct `std::simd` Clifford implementation from `stab-core` instead of allowing Nightly code into the Stable algebra crate. Portable SIMD remains intentionally unavailable until `stab-kernels-simd` owns a dependency-free raw implementation and scalar-versus-SIMD equivalence plus performance evidence justify registering it.

Cross-crate analysis and execution callers use ordinary public algebra operations where the operation is a meaningful safe algebra API. Constructors that deliberately bypass repeated resource admission remain under `stab_algebra::advanced`, making the low-level boundary explicit without exposing storage fields.

Correctness-manifest schema version 5 records 107 cross-crate reexport relationships instead of treating facade paths as independent implementations. Existing exact evidence mappings follow a rustdoc-proven external alias only when the canonical dependency path has no explicit ledger owner, preserving the established `stab-bits` and `stab-records` ownership while transferring algebra facade evidence to the new canonical crate. The regenerated correctness inventory contains 2,886 upstream cases, 5,111 exported API items, 2,003 evidence parents, and digest `5d0054d91e9bb21a662d695884c1eb598226ac9451598c6c021bac27001da4d5`.

The corresponding performance inventory still contains 127 checklist rows, 179 groups, and 167 inherited rows. It now covers all 5,111 API items without adding a speculative workload, and its digest is `41f0545f26529c554e4bf152f40833e7b7928613dd9e699360d551ea3fe3e7e9`. Existing M6 Pauli, iterator, and Clifford-string workload families own both canonical `stab_algebra` paths and their facade aliases.

The scalar algebra package passes `cargo +1.97.1 check -p stab-algebra`, its focused tests, warnings-denied Clippy and rustdoc, the complete `stab-core` compatibility suite, and architecture enforcement. A6 remains open for model, analysis, engine, raw SIMD, facade, feature, ops-contract, fixture, inventory, and benchmark closure.

The first model extraction slice creates the Stable `stab-model` package and moves typed identifiers, probabilities, target values, and exact target parsing into their canonical owner. Direct constructors now return `ModelError`, while `stab-core` retains the established type paths and maps every model construction variant losslessly into its aggregate `CircuitError`. Exact Stim target limits and the parsed `rec[-0]` distinction remain unchanged. Circuit, DEM, gate, parser-diagnostic, resource, and fingerprint ownership remain in the following model slices; this checkpoint does not claim that `stab-model` is complete.

The qualification inventory now treats `stab-model` as an independent rustdoc owner and permits exact selectors from its owner tests. The regenerated correctness inventory contains 2,886 upstream cases, 5,314 public API items, 242 cross-crate aliases, 2,018 evidence parents, and digest `87a198cb2e6d3d65e3021ed6994443d689b38729d0cf9f5b3ae6213f9653e006`. The curated package exposes canonical root values and an explicit `advanced` boundary instead of duplicating every item through public source modules. The regenerated performance inventory retains 127 checklist rows, 179 groups, and 167 inherited rows while covering all 5,314 API items; no benchmark product is added for the type-location change.

The next A6 boundary slice removes the temporary foreign inherent-method adapters from `Circuit`, `DetectorErrorModel`, `Gate`, and `GateDecomposition`. Model values now expose model behavior only; algebraic projections, transforms, compilation, reference sampling, and detector-error analysis are reached through named functions in their logical owner modules. This is a source-level pre-0.2 API migration, not an algorithm change, and the migration inventory records every removed method together with its replacement function.

Semantic tests and benchmark callers use the named owner functions directly. Large transform fixtures retain only private test-local extension traits where method syntax keeps the fixture readable; those traits delegate to the same public owner functions and are not product APIs. Assertions that compared a forwarding method with its own implementation were removed, while independent output, rejection, resource-limit, statistical, and performance coverage remains.

After the adapter removal, the regenerated correctness inventory contains 2,886 upstream cases, 5,286 public API items, 224 cross-crate aliases, 2,011 evidence parents, and digest `f67cb94d5029a4a331f32fc0c16ef2629dfcf379b13c97f8fe3ed55968d8dde4`. Every deleted method owner was rebound to its exact named function; no semantic parent, upstream case, oracle fixture, performance family, or acceptance gate was removed. The performance inventory remains at 127 checklist rows, 179 groups, and 167 inherited rows with digest `cbd3151df95bb9a898bb1537a25dfde30caae14bb5bd5792fdc80894b5c8947b`.

The next boundary removes the `ops-contracts` product feature before moving the complete gate registry. The ordering changed because `GateInfo` embedded qualification-only semantic-family metadata; carrying that field across would have made `stab-model` depend conceptually on operations policy or required a temporary second gate table. The oracle now owns the authoritative gate case set and integer acceptance boundaries, while core retains only test-compiled semantic fixtures.

The benchmark-only analyzer diagnostics are deleted instead of becoming another public API. The PF6 adapter validates nonempty compact repeat structure and exact recurrence detector shifts from public `DetectorErrorModel` output, preserving the period-8 and period-127 workload witnesses without timing or maintaining private counters in product analysis. Existing semantic tests retain exact output, fallback, coordinate, saturation, and resource-limit coverage.

After this ownership repair, the regenerated correctness inventory contains 2,886 upstream cases, 5,286 public API items, 224 cross-crate aliases, 2,009 evidence parents, and digest `66a985cb2710efcdcd3a04d9513507c57e07b94efa203e953b8247d294547ea2`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited rows with digest `87f8bbac381417b37f753109f2412c3856bcbdc000cb3206425004752bdeff1e`. No product API, CLI behavior, benchmark threshold, parity rule, or deferred surface changed.

A dirty report-only production probe at `target/benchmarks/a6-ops-contracts-analyzer-compare` exercises all six cycle-folding submeasurements through the replacement public-output adapter. It is diagnostic only because it records local modifications and has no faithful Stim comparator; the exact runner test independently confirms detector shifts 8 and 127 for the short- and long-period workloads.

The next physical slice moves the closed Stim v1.16.0 gate registry from `stab-core` into the Stable `stab-model` crate. The model now owns canonical and aliased lookup, categories, argument and target rules, syntax validation, inverse metadata, static H/S/CX/M/R decomposition text, raw flow descriptors, and scalar unitary rows. `stab-core/src/gate.rs` is reduced to compatibility reexports, narrow parser delegation, and the test-only semantic surface contract; tableau, flow, unitary-matrix, and decomposition parsing remain analysis operations.

This split deliberately does not move `GateSemanticFamily` into the model. That classification exists to organize selected execution and qualification behavior, not to describe Stim syntax. It is now derived exhaustively by the core test contract, which keeps the canonical product table independent of operations policy and avoids a second registry. Gate construction and validation return `ModelError` at the canonical boundary, while existing facade parser and circuit APIs preserve `CircuitError` through the lossless aggregate conversion.

The exact 82-name Stim hash corpus moves to the canonical model owner. New model tests cover aliases, metadata, validation failures, and raw descriptor availability; all 48 semantic-contract tests and the broader core gate tests remain green. The regenerated correctness inventory contains 2,886 upstream cases, 5,413 public API items, 230 cross-crate aliases, 2,009 evidence parents, and digest `3f40774245267a3f7305fe0ff668dd458b7a7df069b40da500bd00f28906c55f`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited rows with digest `824df0bdd1bfc776f82ab8bbd05752169cee26f268e317152ac2cdb0f0bacc49`; no benchmark obligation is added for the ownership move.

The source-current M4 comparison at `target/benchmarks/a6-gate-model-m4-clean-fa62c935` records commit `fa62c935` with `local_modifications=false`. Dense circuit parsing is `0.5624x` pinned Stim, sparse circuit parsing is `0.5538x`, and canonical gate hashing is `0.3821x`; the ownership move therefore retains the existing `1.25x` gate without a waiver or threshold change.

The next model-boundary prerequisite removes filesystem policy from `Circuit`. The Stable model surface keeps byte parsing and canonical writer output, while facade-owned `read_stim_circuit_file` and `write_stim_circuit_file` retain the exact 64 MiB read cap, streaming output, and aggregate I/O errors. This is a deliberate 0.2 source migration rather than a behavior change, and the exact file-helper qualification parent now owns the two facade functions.

An independent post-extraction review then found that the newly public gate validators were attached to a metadata-accessor parent that did not execute them, the performance runtime owner still named `stab-core/gates`, and the model package claimed but did not declare its Stable MSRV. The validators now have their own exact Stable-package selector and negative axes, the M4 runtime owner is `stab-model/gates`, and `stab-model` declares Rust 1.97.1 in package metadata. The file-helper regeneration also replaced the review's stale intermediate manifest identity; the source-current correctness inventory contains 2,011 evidence parents with digest `883bd4a5147bbc8b69af56535a460b4b01c0e38b8930d34c0cb280765e035bfd`, and the performance inventory digest is `02e5a637b8b76713739542259388b0076c732168dc9899d89bcd1780b4c2d247`.

Core transforms and execution preparation previously reached through five crate-private `Circuit` storage operations. They now construct through one fallible `CircuitAssembler` with explicit unfused input, exact reservation, fused instruction append, repeat append, and finalization. This keeps allocation failures and instruction-boundary policy typed at one advanced seam before the value moves, instead of making raw model fields or several unrelated mutation hooks public across the future crate boundary.

The next model slice moves shared parser diagnostics, model dialect identity, parser limits, and the honest resource-estimate vocabulary into `stab-model`. `stab-core` now reexports those canonical values and converts `ModelError::Parse` losslessly into `CircuitError::Parse`; result-format diagnostics and operation-specific resource-limit contexts remain with their actual records and facade owners.

The placement is dependency-driven rather than cosmetic. Circuit and DEM parsing, fingerprints, analysis, and execution all need the model vocabulary, while the Stable model package cannot depend on the Nightly facade. Conversely, `stab_records::EncodedSizeEstimate` stays records-owned: once `Estimate` becomes model-owned, Rust's orphan rules prevent retaining the old facade-local generic `From` implementation, and adding a model-to-records dependency would invert the target graph. The two internal composition sites now convert `Exact` and `Unknown` explicitly.

Temporary constructors under `stab_model::advanced` let the still-unmoved core parser and fingerprint code construct checked spans, bounded diagnostics, dialect discriminators, and aggregate estimates without exposing private fields. These are narrow behavior-oriented seams, not storage access, and they leave with or shrink after the circuit, DEM, and fingerprint implementations move.

Direct Stable-package tests cover byte-span overflow, malformed UTF-8 location, typed parser context, dialect order and names, parser defaults and hard ceilings, exact LF/CRLF and opaque-byte estimates, and unknown dimensions. A facade test independently proves that model parser errors and parser policies retain complete `CircuitError` payloads and established human diagnostics. The complete `stab-core` suite, targeted CLI agent tests, warnings-denied Clippy and rustdoc, and architecture enforcement pass for this slice.

The regenerated correctness inventory contains 2,886 upstream cases, 5,637 public API items, 2,011 evidence parents, and digest `0312ad9e79bf72ad361a4869a4d415f852e82940f0beb325bbd669284d0d683f`. The performance inventory remains at 127 checklist rows, 179 groups, and 167 inherited decisions with digest `b5dec487219e3fdab51847c28fb391ce1a2968afb357fa7c9778ba7ff5fd7567`; no benchmark product, comparator, threshold, or waiver was added for this ownership-only move.

The following prerequisite moves the four real circuit and DEM parse-resource causes into the Stable model package. Source-line and repeat-depth failures retain exact operation, dimension, actual, limit, span, source-line context, and human display; `stab-core` converts the closed context exhaustively into its broader analysis-and-execution resource family. Both existing parsers now exercise the model constructors, so the seam is not a placeholder. A new direct model qualification parent owns the canonical values and constructors, while the existing circuit and DEM policy cases continue to prove facade compatibility.

After this prerequisite, the correctness inventory contains 2,886 upstream cases, 5,704 public API items, 2,013 evidence parents, and digest `4f216cc289df2ba78abe523721f7309fbc060e9e6f759772af9f55d1d1bfe6fe`. The performance inventory remains at 127 checklist rows, 179 groups, and 167 inherited decisions with digest `832693ba6d578043863edbdf091dafa41ca34b9bd35fcc3273d5aa93b6e63f17`; the new diagnostics are classified as resource correctness and do not create a timing product.

The next ownership prerequisite separates structural validation from parsing and resource admission. `stab-model::ValidationError` now owns the closed unknown-gate, domain-value, argument, and target failure set, and `ModelError::Validation` aggregates it beside parse and resource failures. `stab-core` exhaustively converts every validation cause back into the established `CircuitError` variant, so this boundary changes component ownership without weakening facade compatibility. A semantic model-only test constructs real oversized-ID and unknown-gate failures and proves callers can distinguish the validation family without parsing diagnostics.

The validation family also owns structural count, coordinate, and detector-query failures already produced by the Circuit API. Stable codes and severity make these failures machine-readable, while facade conversion preserves the former result-format or detector-model error class and exact human prefix. The regenerated correctness inventory contains 2,886 upstream cases, 5,791 public API items, 2,014 evidence parents, and digest `aaccf0f43cf8d9ce835a6fb3eba467e0e890120b5b4514f26b28b8a6ccd74d86`. The performance inventory remains at 127 checklist rows, 179 groups, and 167 inherited decisions with digest `2a93cf312f51b814658f47e07fe0f89dbee94b9dd40f26816d827220e8db2d19`; this error-boundary change does not add or alter a timed workload.

The final pre-move traversal prerequisite removes facade operation identity from `DemRepeatSelection::Expand`. The model traversal now owns only the cumulative ceiling and readable context, while each visitor constructs its own limit failure. Logical-error search and SAT retain their exact typed `ResourceOperation`, `ResourceKind`, values, and human diagnostics through visitor overrides; ErrorMatcher keeps the default model-style validation diagnostic. Existing traversal, logical-search, SAT, and ErrorMatcher resource suites prove that this dependency inversion does not weaken admission.

The complete model implementation is now physically extracted. `Circuit`, `DetectorErrorModel`, their instructions and repeat blocks, exact byte parsers, canonical text and byte writers, structural iterators and queries, compact folded DEM traversal, opaque metadata, shared parser support, and schema-one model fingerprints exist only under `stab-model`; the corresponding `stab-core` modules are reexport facades or analysis-owned consumers. Filesystem opening and truncation policy deliberately remains facade-owned, while transforms, search, circuit-to-DEM analysis, reference sampling, and execution remain outside the model for the later analysis and engine slices.

The extraction keeps private storage private. Core transforms construct circuits through one checked `CircuitBuilder`, and analysis and execution consume a public advanced folded-DEM traversal whose callbacks expose semantic state without materializing repeats. New model-owned tests prove compact summaries, detector and coordinate shifts, every repeat selection, selected-index rejection, prompt visitor cancellation, exact expansion admission, and the consumer-owned limit-error hook. The CLI converts `ModelError` back through `CircuitError`, preserving its established human and JSON diagnostic families despite the canonical parser move.

Correctness ownership now follows the canonical model paths, with facade paths recorded as aliases instead of duplicate implementations. The regenerated inventory contains 2,886 upstream cases, 6,119 public API items, 2,037 evidence parents, and digest `4d58d030d7b871cbda7740cf36a5c2e87a58cce082e85ceb41faf04b60aa7bd6`. M4 circuit parse and canonical print plus M10 DEM parse and canonical print workers call `stab-model` directly; workload IDs, semantic inputs, comparator sources, parity rules, regression baselines, and the `1.25x` ceiling are unchanged. The regenerated performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `25285a77ebbf0c62bcf80d2502fe4743d785723aea545f7f4b25962165b8b45e`.

Stable Rust 1.97.1 model tests, warnings-denied Clippy, rustdoc, the complete core suite, exact selector validation, and correctness regeneration pass for this source slice. The focused extraction is clean commit `a02ee939ad982f8747e3ab52430390e90187a838`.

Clean-revision legacy evidence is under `target/benchmarks/a6-model-extraction-baseline-a02ee939`, `target/benchmarks/a6-model-extraction-compare-a02ee939`, and `target/benchmarks/a6-model-circuit-parse-gate-a02ee939`, all with `local_modifications=false`. Dense and sparse M4 circuit parsing report `0.554x` and `0.499x` pinned Stim and pass the unchanged `1.25x` gate; canonical circuit printing reports 192 ns per representative operation as contract-only evidence. The representative M10 rows report 112 ns for DEM parsing and 80 ns for canonical printing, but retain their source-owned representative or contract-only classifications rather than claiming a direct Stim ratio.

The source-owned adapter probes report circuit parse at `1.025x`, circuit print at `0.478x`, and DEM print at `0.693x` pinned Stim for 524,288 work items. The Stim-first DEM parse smoke reports `1.802x` at 524,288 work items and `1.412x` at 2,097,152 work items, so it is retained only as order-sensitive diagnostic output and is not promoted as parity evidence. A six-pair alternating self-comparison at 2,097,152 work items records current Stab median `0.2009 s` versus immediate parent `dfcf5893` median `0.2172 s`, a `0.925x` extraction self ratio; the model move therefore has no unexplained Stab-side parse regression. Formal Stim parity remains owned by the paired controlled-host qualification runner rather than this one-order probe.

A6 now proceeds to the Stable `stab-analysis` extraction.

The first analysis slice creates a real Stable Rust 1.97.1 `stab-analysis` package over `stab-model` and `stab-algebra`. It physically owns the gate semantic bridge, including single-qubit Clifford lookup, local gate tableaus, gate flows, fixed-shape unitary matrices, H/S/CX/M/R decomposition lowering, and capability predicates, plus recursive structure-preserving circuit tag removal. `stab-core` retains thin compatibility wrappers and converts the closed analysis error family losslessly into its established aggregate `CircuitError`.

The slice intentionally does not claim the complete analysis component. Circuit simplification, transforms, flows, generation, inversion, feedback, detecting regions, missing-detector analysis, circuit-to-DEM lowering, DEM flattening and search, SAT, error matching, MBQC, and sparse reverse tracking remain in `stab-core` and are named explicitly in the extraction map. `AnalysisError` exposes only failures that the moved functions can produce; placeholder variants for later families were rejected to avoid freezing speculative API.

Qualification now inventories `stab-analysis` directly, accepts exact Stable-package selectors, and treats curated root exports as canonical while recording public module paths and `stab-core` facade paths as aliases. Seven semantic parents own every current analysis API, including independent pinned gate-table tableau and flow fixtures, exact matrix and decomposition contracts, the exhaustive Clifford map, recursive tag removal, and a typed-error case. The regenerated correctness inventory contains 2,886 upstream cases, 6,184 public API items, 2,038 evidence parents, and digest `926bc8d5bd6016c071bc06f61cc22a24728d26e0bc5da04fe10cfcf731581bcc`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `acf10c4b76fa61e04a414309d109c2e195dd878e62092bad0c3b4779edc5a603`; no new timing product, threshold, or waiver was added for this ownership-only slice.

The focused extraction is clean commit `6408f367`. Clean report-only benchmark evidence is under `target/benchmarks/a6-analysis-gate-metadata-compare-6408f367` and `target/benchmarks/a6-analysis-gate-semantic-compare-6408f367`. The metadata workload reports 7.39 million tableaus/s, 7.37 million flows/s, 539 million fixed-shape matrix entries/s, 12.1 million parsed decomposition instructions/s, and 492 million alias lookups/s. The broad gate-semantic workload separately reports sampler execution, reference sampling, converter compilation, detection sampling, detector-frame sampling, error analysis, and flow generation; it preserves those phase identities and makes no aggregate or pinned-Stim ratio claim. Both rows retain their existing contract-only classification, and no threshold, waiver, comparator, or workload identity changed.

The next compiling slice moves the coupled full-circuit tableau and simplification implementations together. This ordering avoids a forbidden temporary `stab-analysis -> stab-core` edge because tableau conversion lowers Pauli-product instructions through the single-instruction decomposition seam. The canonical Stable crate now owns `circuit_to_tableau`, `simplified_circuit`, `decomposed_circuit`, and the engine-facing advanced single-instruction lowering; `stab-core` keeps only aggregate-error wrappers and its string-name gate-tableau helper for still-unmoved analyzer callers.

The complete tableau and simplification test files move to the canonical crate, while broader core integration tests continue to prove facade error conversion and use from flows, inversion, transforms, missing-detector analysis, sampling, and simulator cross-checks. A direct anti-Hermitian decomposition test owns the new `InvalidCircuitSimplification` outcome. The regenerated correctness inventory contains 2,886 upstream cases, 6,189 public API items, 2,038 evidence parents, and digest `d1291c468119c382c6970b9dc99bf832f1b89a11ed91cb902e60f2f362cf9069`. The performance inventory remains at 127 checklist rows, 179 groups, and 167 inherited decisions with digest `4136a8d75075d63a537385712bc99fd867a94bff48143a2936181f27efdcca26`.

The focused source commit is `efc666f6b0840bd957a32829b6224a6b1adde3da`. Clean-revision diagnostic evidence is under `target/benchmarks/a6-analysis-circuit-tableau-efc666f6-compare` and `target/benchmarks/a6-analysis-circuit-decompose-efc666f6-compare`, both with `local_modifications=false`. The existing deterministic 32-qubit tableau workload reports 23.08 thousand source gates/s for circuit conversion, 236.3 thousand qubits/s for inversion, and 6.309 million qubits/s for Pauli application. The decomposition workload reports 121.5 thousand source instructions/s over ISWAP, MPP, SPP, pair-measurement, noise, and annotation operations.

These probes retain their source-owned report-only and contract-only classifications. The Stim tableau measurements use different random-width and CNOT workloads, while the decomposition row has no faithful direct Stim comparator in this harness. The evidence therefore validates that the moved production paths remain executable and measured without adding a parity ratio, threshold, waiver, or comparator claim.

The next analysis slice moves bounded circuit flattening and noise removal into the Stable canonical crate. `CircuitFlattenLimits` and the six real circuit-flatten resource causes now live beside the transform that enforces them; the analysis error exposes typed operation, dimension, actual value, limit, and unchanged human display, while the core aggregate wraps that error losslessly. The owning six-case resource suite moves to `stab-analysis`, and a separate core facade case proves the old `CircuitError` resource class and fields remain unchanged.

The regenerated correctness inventory contains 2,886 upstream cases, 6,266 public API items, 2,038 evidence parents, and digest `f568b21cad0700e06af7da1f550be46c5a48862a3fdc02a412fbdc27c73b61d2`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `72dc3e9e0004675cc582c2a3539b74548ef78f33fb36723043432610e4f975bc`; the move changes ownership and call paths without adding a benchmark product, threshold, waiver, or comparator.

The focused source commit is `46ada3bb`. Clean-revision diagnostic evidence is under `target/benchmarks/a6-analysis-flatten-46ada3bb-compare` and `target/benchmarks/a6-analysis-without-noise-46ada3bb-compare`, both with `local_modifications=false`. Bounded repeat flattening with coordinate shifts reports 21.42 million expanded operations/s, and top-level noise removal reports 39.89 million source instructions/s. Both rows retain their contract-only classification because the harness has no faithful direct pinned-Stim comparator for these exact Rust API workloads.

The following analysis slice moves repetition, surface, and color-code generation plus MBQC decomposition into the Stable canonical crate. Generation belongs in analysis because it is a deterministic model-to-model construction with explicit domain and materialization admission, while MBQC decomposition is a pure gate-to-circuit lowering; neither operation owns random state, mutable execution sessions, record codecs, filesystems, or operational policy. Moving them together also keeps their shared gate-lowering dependency below the future engine boundary.

`stab-analysis` now owns the parameter types, generated-circuit value, complete family implementations, helper semantics, resource admission, and pinned compatibility suites. `stab-core` retains explicit wrapper types instead of aliases because its public constructors and generators must preserve `CircuitResult`, `CircuitError::InvalidDomainValue`, and the established generated-value API. Two focused facade tests prove lossless resource rejection with constant scratch and zero-allocation fixed-size parameter construction, while the broader no-noise detector and observable matrix remains in core because it crosses into sampling and circuit-to-DEM execution.

Correctness ownership follows the canonical analysis selectors while preserving facade API owners and pinned Stim fixture links. The regenerated inventory contains 2,886 upstream cases, 6,374 public API items, 2,040 evidence parents, and digest `a5235e781da44994ce3e872c0b469ddc80cd8e4770b75e70f4755484b712aca5`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `5b8d571847a6c007fdd8168d0f85510db42cadbe0b834734d9ee4f4a90e2aa39`; workload contracts, comparators, parity thresholds, regression baselines, and waivers are unchanged.

The focused implementation commit is `b82c5e04`, and qualification ownership is synchronized at `f0d785e6`. Clean-revision M7 evidence is under `target/benchmarks/a6-analysis-generation-f0d785e6-baseline` and `target/benchmarks/a6-analysis-generation-f0d785e6-compare`; the comparison records commit `f0d785e61da40baac5c51b5c6dc87053a49a7525` with `local_modifications=false` and exercises all 23 source-owned generation rows. Repetition generation ranges from 1.248 microseconds for distance three to 5.584 microseconds for distance seventeen, rotated surface generation ranges from 7.856 to 319.437 microseconds, unrotated surface generation ranges from 10.576 to 129.055 microseconds, and the distance-five color case takes 13.040 microseconds. These rows remain report-only because Stab measures direct typed construction while pinned Stim is executed through its CLI, so the evidence records moved-path health without claiming a Stim ratio. MBQC decomposition remains a source-owned future performance candidate and does not gain a speculative benchmark product merely because its implementation moved.

The next analysis slice moves the complete unsigned circuit-flow checker, generator, measurement solver, transition table, and sparse reverse-frame tracker together. The dependency is structural: flow generation and checking use the same reverse transitions and tracker state that later DEM analysis consumes, while none of these operations owns random execution, result codecs, filesystems, or ops policy. Keeping the coupled algorithms in one Stable owner avoids either duplicating transition semantics or exposing facade-private model storage across crates.

Repeat-contained flow generation initially arrived with a temporary facade-supplied flattened-instruction seam because the isolated worktree predated bounded flatten extraction. Integration removed that seam and the duplicate hard-coded one-million-operation limit; canonical analysis and facade callers now both use `flattened_circuit_operations` and receive the same typed `CircuitFlattenLimits` failures. The core flow facade delegates directly and preserves the established aggregate error class.

The full generator and PFM-B4 evidence suites move to `stab-analysis`; core retains cross-component flow integration tests for inversion, feedback, detecting regions, missing-detector analysis, and execution. All 33 flow-engine blocker cases plus the shifted and unitary large-repeat tracker cases now resolve through exact Stable-package selectors. The regenerated correctness inventory contains 2,886 upstream cases, 6,490 public API items, 2,078 evidence parents, and digest `b673242fe4cc7e55b79c79df01b79bdf5600b0bd0ab500e24648754c84869788`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `1abdcddc7b3cefe5840a33d91b3371a029eef5252684c0b0c4646c987b263a9a`; no comparator, threshold, waiver, or workload identity changes.

The focused implementation commit is `abcaffdf`, and qualification ownership is synchronized at `cc482587`. Clean-revision PF5 evidence is under `target/benchmarks/a6-analysis-flow-cc482587-baseline` and `target/benchmarks/a6-analysis-flow-cc482587-compare`; the comparison records commit `cc4825876cecc1ebef41e1b805070cf9c6ffe7f9` with `local_modifications=false` and exercises all 14 PF5 analysis rows, including the five moved flow rows and their neighboring reverse-tracker consumers. The moved flow solver rows range from 70.85 to 92.14 thousand cases per second, the flow-generator rows range from 159.5 to 285.2 thousand cases per second and 948.5 thousand to 1.277 million flows per second, and the batch checker records 370.6 thousand cases per second and 2.501 million flows per second.

Clean-revision sparse-tracker evidence is under `target/benchmarks/a6-analysis-tracker-cc482587-baseline` and `target/benchmarks/a6-analysis-tracker-cc482587-compare`. Its dedicated PF6 row records 133.0 billion folded rounds per second for the compact unitary repeat, 109.6 million idle qubits per second for the 65,536-wide sparse case, and 31.79 million folded rounds per second for shifted measurement tracking. Every cited row remains report-only or contract-only because pinned Stim has no faithful matching in-process comparator for these typed Rust workloads, so this evidence establishes moved-path health and preserved semantic work without claiming a Stim-relative speed ratio.

The inversion slice moves unitary inversion, the implemented QEC inverse packets, and tracker-driven flow reversal after their shared flow transitions and sparse tracker. This ordering removes the last private flow dependency from core inversion without coupling pure analysis to result records or execution sessions. Pure inversion suites move with the implementation; the PFM-B1 generated-surface suite and opaque-tag regressions remain in `stab-core` because they intentionally prove cross-feature and facade behavior.

The core facade retains independent `InverseQecOptions` and `TimeReversedForFlowsOptions` DTOs instead of reexporting analysis-owned types through two public paths. This small adapter prevents alias-to-alias canonical ownership, preserves the established root and `analysis` namespace APIs, and keeps error conversion explicit. The focused implementation commit is `7ee82540`, followed by the facade-identity correction at `0ea590e8`.

All M6 and PF2 oracle rows pass after retargeting the pure selectors to `stab-analysis`, while PFM-B1 cross-feature selectors continue to resolve from `stab-core`. The regenerated correctness inventory contains 2,886 upstream cases, 6,513 public API items, 2,085 evidence parents, and digest `4810270ad3cb1183d13c293ef74f3ec4938ead76a8130d85dcf72409f2ac0d68`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `d02432bd650dc5e564681420b7fec399999f9e78a0ab09b5a3de891669bbc151`; no comparator, threshold, waiver, or workload identity changes.

Clean-revision PF2 evidence is under `target/benchmarks/a6-analysis-inversion-98e8063d-baseline` and `target/benchmarks/a6-analysis-inversion-98e8063d-compare`; the comparison records commit `98e8063dd89550cdafa59d93ab4aa9e3efe6a147` with `local_modifications=false` and exercises all ten PF2 transform rows. The scoped unitary and measurement-rich inverse rows record 192.2 thousand and 1.025 million flows per second, the generated-surface matrix ranges from 1.395 to 2.294 million source instructions per second, the MPAD scale matrix reaches 2.275 million flows per second, and the million-index sparse case records 1.071 million transforms per second without width-amplified allocation. Every row remains contract-only because pinned Stim has no faithful matching in-process comparator, so this evidence establishes extraction health without claiming a Stim-relative ratio.

The A6 SIMD audit found no executable `std::simd` site in the current product graph. Scalar extraction had already removed the former packed-bit and Clifford implementations, so the kernel milestone is corrected from a mechanical move to a deliberately small restoration: dependency-free four-word bit and Clifford kernels, scalar differential references, scalar defaults, explicit feature identity, and backend registration only after distinct semantic and performance evidence. Popcount, transpose, sparse XOR, Pauli-phase multiplication, tails, allocation policy, and sampler backend policy remain outside the initial raw-kernel API.

The feedback slice follows inversion because both consume the extracted sparse reverse-frame tracker, but feedback lowering does not need the circuit-to-DEM analyzer itself. The canonical implementation, bounded repeat checks, loop refolding, and exact-output tests move to `stab-analysis`; core keeps its one-function `CircuitError` facade plus the cross-feature DEM-equivalence and public API tests. Existing fixed feedback ceilings continue to return the established invalid-DEM error class, avoiding an unrelated resource-contract change during extraction.

The focused implementation commit is `63cce91a`. The M9 ownership row now executes the canonical analysis tests, while PF2 facade rows continue to execute core integration tests. The regenerated correctness inventory contains 2,886 upstream cases, 6,514 public API items, 2,086 evidence parents, and digest `737cecc89e7a7622dff9df0b9ca2e208d55b844aa62b7adf727c9988895b6459`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `40d26d39f270ed43316324d8f246ac76fd1adb4098333b657fa8dfe19663bcef`; no comparator, threshold, waiver, or workload identity changes.

Clean-revision feedback evidence is under `target/benchmarks/a6-analysis-feedback-a8fd49d7-baseline` and `target/benchmarks/a6-analysis-feedback-a8fd49d7-compare`; the comparison records commit `a8fd49d75e2b6086abf9daaf9ca42275f67b2d66` with `local_modifications=false`. The scoped row records 350.6 thousand MPP transforms per second, 547.3 thousand repeat iterations per second, and 691.6 thousand XCZ/YCZ transforms per second. It remains contract-only because pinned Stim has no faithful matching direct Rust baseline.

Detecting-region and missing-detector analysis move next because they are the last circuit-only consumers of the extracted flow and reverse-tracker foundations. Both are deterministic model-to-report operations with no record codec, execution session, filesystem, CLI, or ops dependency. Moving them before the DEM analyzer keeps the next extraction boundary honest: `stab-analysis` now owns all implemented pure circuit analysis, while the remaining work is entirely DEM analysis, DEM transforms, search, SAT, and error matching.

The canonical implementations, folded-repeat logic, reusable work maps, options, and owning semantic tests now live in `stab-analysis`; `stab-core` retains explicit wrappers that preserve `CircuitResult`, facade-owned option DTOs, and the established public namespace. Cross-component CZ, sweep, generated-surface, DEM-equivalence, and PFM-B4 tests remain in core because they intentionally prove facade composition rather than just the moved algorithms. The focused source commits are `d7d2fb51` for detecting regions and `a6d5b298` for missing-detector analysis.

Qualification retargets pure ownership selectors to `stab-analysis` while retaining the cross-component core selectors. All M9 and PF5 implemented oracle rows pass under that split. The regenerated correctness inventory contains 2,886 upstream cases, 6,547 public API items, 2,091 evidence parents, and digest `265ec8d9eb6682b3aadd1b2d0bb4233cd52c42bf535170243aabc1cb1d13413c`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `d07a2d0e9e4b138c02ec609892707e9ab5bb70d23bb7e38852f7564a88350c7b`; no workload, comparator, parity threshold, regression baseline, or waiver changes.

Clean-revision PF5 evidence is under `target/benchmarks/a6-analysis-detector-utils-2d91b997-baseline` and `target/benchmarks/a6-analysis-detector-utils-2d91b997-compare`; the comparison records commit `2d91b997c59125fd574ea406d5cf6205e17111a5` with `local_modifications=false` and measures all 14 selected rows. Detecting-region repeat and target-filter workloads record 747.0 thousand and 844.8 thousand cases per second, representative Clifford propagation records 241.5 thousand cases per second, generated repetition and rotated-surface workloads record 86.83 thousand and 43.70 thousand cases per second, missing-detector MPP records 854.9 thousand cases and 641.5 thousand suggestions per second, MPAD records 2.928 million cases and 2.930 million suggestions per second, and the complete honeycomb and toric generated-code workload records 540.7 cases and 541.2 suggestions per second. The neighboring flow rows also complete, including the 512-by-1024 sparse solver matrix. Every cited detector-analysis row remains report-only or contract-only because pinned Stim has no faithful matching CLI timing ratio for these typed Rust APIs.

The circuit-to-DEM slice moves after the circuit-only analyses because it consumes the same extracted gate semantics and sparse reverse tracker but remains a pure model-to-model operation. The canonical Stable crate now owns analyzer options, direct and folded lowering, recurrence probing, gauge handling, error decomposition, and the independent/disjoint XYZ conversion values. It imports only model and algebra dependencies, calls model-owned advanced DEM constructors directly, and preserves the established invalid-DEM error class for fixed analyzer ceilings instead of recasting them as configurable resource admission.

All 81 pure `dem_analyzer_*` integration cases and ten analyzer unit cases move with the implementation. Core keeps two focused facade regressions for exact folded output and lossless `CircuitError` conversion, plus generated-QEC semantic equivalence and CLI composition tests. The source commit is `7608b70a`; the obsolete analyzer-only core tableau and sparse-tracker shims are removed because no core implementation consumes them.

Qualification selectors preserve their exact test names while retargeting package and internal module ownership to `stab-analysis`. The regenerated correctness inventory contains 2,886 upstream cases, 6,581 public API items, 2,094 evidence parents, and digest `999411729ac3297c0247d44c8d64babc12dcbc55208be8e94fb25e37058ce28f`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `a8ac03892be5def364c6309200fe0763e6d2b6b564a9333fef1dec33a3dcf28e`; the M10 analyzer and XYZ conversion runners call the canonical crate directly, while workload descriptors, semantic work, comparator classes, parity thresholds, regression baselines, and waivers remain unchanged.

The qualification ownership commit is `8def1009`. Clean-revision M10 evidence is under `target/benchmarks/a6-analysis-circuit-to-dem-8def1009-baseline` and `target/benchmarks/a6-analysis-circuit-to-dem-8def1009-compare`; the comparison records commit `8def1009de178578d0882c2bb05a197f3957e26f` with `local_modifications=false`. The representative surface-code analyzer records 7.675 thousand detectors per second and `0.0130x` its broader pinned Stim row. The exact `m10-error-decomp` gate remains healthy: its source-owned approximate-p10 submeasurement records `0.0111x` pinned Stim, while the row's conservative aggregate ratio is exactly `1.25x`. The decompose, folded-repeat, and high-repeat CLI contracts record ratios below `0.001x`; the DEM parse and print rows retain their representative and contract-only classifications.

Focused clean-revision report-only evidence is under `target/benchmarks/a6-analysis-circuit-to-dem-pf3-8def1009-compare` and `target/benchmarks/a6-analysis-circuit-to-dem-pf6-8def1009-compare`. Selected sweep-controlled analyzer cases range from 868.1 thousand to 1.488 million circuits per second, and folded error decomposition records 1.243 billion represented rounds per second. These rows have no faithful direct pinned Stim timing comparator in the harness and therefore make no Stim-relative claim.

The attempted filtered threshold run remains preserved at `target/benchmarks/a6-analysis-circuit-to-dem-8def1009-threshold`. Its selected M10 rows all evaluate as threshold passes, but the command exits nonzero because the source-owned threshold file also contains unselected milestones. It is retained as a failed diagnostic and is not promoted as successful gate evidence.

The DEM-transform slice moves recursive tag stripping, bounded materialized flattening, and probability rounding after circuit-to-DEM analysis because these are deterministic model-to-model operations with no search, SAT, matching, record-codec, execution-session, filesystem, CLI, or ops dependency. `DemFlattenLimits` and its typed retained-payload, repeat-work, and materialized-byte failures move beside the canonical flatten implementation; `stab-core` retains thin wrappers and the established aggregate `CircuitError` conversion. The focused source commit is `8af5c4a1`.

Eight meaningful flatten-policy tests and ten exact transform tests now execute from `stab-analysis`, including pinned compact-transform behavior, opaque tags, folded repeats, exact boundaries, overflow, and admission-before-materialization. Core retains mixed introspection, execution, and facade coverage plus one focused lossless error-conversion case. The previous standalone builder/getter derive-style test was intentionally removed because it restated field plumbing without protecting a Stim behavior, resource invariant, or facade contract.

Qualification selectors and public API ownership now point at the canonical Stable crate while preserving the existing root and `analysis` facade paths as aliases. The regenerated correctness inventory contains 2,886 upstream cases, 6,614 public API items, 2,094 evidence parents, and digest `f7515d4b8e1ee765ccf19208f86b53c8651b7922a724e4cd952c88baa6a19e05`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `66f15f1242ad2c6d1b58256d6b3a01344a254bf2e9c1bfe751a742ff5f9c8874`; the PF1 and PF4 runners call `stab-analysis` directly, while workload descriptors, semantic work, comparator classes, parity thresholds, regression baselines, and waivers remain unchanged.

The qualification ownership commit is `05fc6b8a`. Clean-revision diagnostic evidence is under `target/benchmarks/a6-analysis-dem-without-tags-05fc6b8a-{baseline,compare}`, `target/benchmarks/a6-analysis-dem-flatten-05fc6b8a-{baseline,compare}`, and `target/benchmarks/a6-analysis-dem-rounded-05fc6b8a-{baseline,compare}`; every comparison records commit `05fc6b8a899033cea68f803b10d9a77e6b83daef` with `local_modifications=false`. Recursive tag stripping reports 4.274 million queries per second, repeat flattening reports 16.99 million expanded instructions per second, and compact probability rounding reports 15.21 million probability arguments per second. All three rows retain their source-owned contract-only classification because pinned Stim exposes equivalent behavior without a faithful direct Rust comparator.

The SAT slice moves next because shortest-error and weighted WCNF generation are deterministic DEM analysis over the model-owned folded traversal boundary. The canonical implementation, compressed target indexing, exact CNF preflight, serialization, `SatMaterializationLimits`, and typed traversal, mechanism, target, shape, and output failures now live in `stab-analysis`; `stab-core` retains four thin root-function adapters, the established type alias, and lossless aggregate-error conversion. The focused source commit is `70e23359`.

Thirty-two SAT-owned tests move with the implementation: exact pinned WCNF cases, CNF-instance arithmetic, default and every-dimension resource admission, folded flat and nested repeats, sparse identifiers, deterministic and zero-probability behavior, overflow, and source immutability. Generated-QEC WCNF, mixed graph/hypergraph/search resource, and folded traversal umbrella tests remain in core because they prove facade composition across components. A focused facade case independently proves exact output delegation and preservation of the SAT resource operation, resource kind, values, and human diagnostic.

Qualification retargets the five SAT resource selectors and two SAT-only folded-repeat fixtures to `stab-analysis`, while keeping mixed integration selectors in core. The regenerated correctness inventory contains 2,886 upstream cases, 6,659 public API items, 2,097 evidence parents, and digest `55abe5c17b6f9b10eb5af4f88b64b2e82094d75982a165de67167d1fde2ec8af`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `6180b5bcbb502d0ca5de3576c55739e04752cbb88cb2336ccc593c7258885ad1`; PF4 and PF6 SAT measurements now call the canonical Stable crate directly, while workloads, semantic work, comparator classes, parity thresholds, regression baselines, and waivers remain unchanged.

The qualification ownership commit is `65e9f8fc`. Clean-revision SAT evidence is under `target/benchmarks/a6-analysis-sat-fold-65e9f8fc-{baseline,compare}`, `target/benchmarks/a6-analysis-sat-direct-65e9f8fc-{baseline,compare}`, and `target/benchmarks/a6-analysis-sat-generated-65e9f8fc-{baseline,compare}`; every comparison records commit `65e9f8fcaf103b7b49aba52c7e5b81490e2ef8f5` with `local_modifications=false`. The direct DEM workload reports 9.211 million clauses per second for shortest-error WCNF and 7.184 million clauses per second for weighted WCNF, while the generated-QEC workload reports 8.744 million and 7.696 million clauses per second respectively. The folded-repeat row also exercises flat, zero-probability, nested, and weighted SAT materialization. It retains its contract-only classification, and both WCNF rows retain their report-only classification because the harness has no faithful pinned Stim performance filter for these exact typed Rust workloads.

The logical-search slice moves the shared compact detector index, folded nonzero-error traversal, `LogicalErrorSearchLimits`, graph-construction admission, graphlike search, and hypergraph search together. This coupled move avoids either duplicating common traversal and budget logic or exposing a temporary public bridge solely so the still-unmoved hypergraph implementation can call graphlike fallback. Canonical analysis errors own every traversal, graph, hyperedge, and frontier cause, while `stab-core` retains four thin root-function adapters, the established policy alias, and exhaustive aggregate resource conversion. The focused source commit is `a1c62b5d`.

All pure inline graphlike, hypergraph, traversal, and budget tests move with the implementation. Core retains generated-QEC, mixed search, facade resource, and root-entry tests because they prove compatibility composition rather than private graph representation. Broad dead-code exemptions were removed; test-only graph constructors are compiled only for tests, and one unused helper was deleted. The complete Stable analysis suite, complete core suite, warnings-denied Clippy and rustdoc, architecture enforcement, exact logical-search selectors, and source-owned blocker ledger all pass.

Qualification retargets twelve logical-search resource selectors and eleven pinned structural fixture rows to `stab-analysis`, while the core public-entry selector continues to own lossless `CircuitError` conversion. The regenerated correctness inventory contains 2,886 upstream cases, 6,711 public API items, 2,099 evidence parents, and digest `6edb3a17215ab496eb662e1f923a0eeb00df661324a098e1e85dc3be0ea20eec`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `2059b48428404bc706ac9568d89903aba5479a5fd20692a76b6248e84df18463`; M10, PF4, and PF6 search measurements call the canonical Stable crate directly, while workloads, semantic work, comparator classes, parity thresholds, regression baselines, and waivers remain unchanged.

The qualification ownership commit is `93257aff`. Clean-revision search evidence is under `target/benchmarks/a6-analysis-search-m10-93257aff-{baseline,compare}`, `target/benchmarks/a6-analysis-search-pf4-93257aff-{baseline,compare}`, `target/benchmarks/a6-analysis-search-graphlike-d25-93257aff-{baseline,compare}`, and `target/benchmarks/a6-analysis-search-hypergraph-qec-93257aff-{baseline,compare}`; every comparison records commit `93257aff55bf29a8754187f82267c509fc3fa495` with `local_modifications=false`.

The representative chain row records 3.537 million graphlike edges per second, the folded traversal row records 1.300 million expanded errors per second plus compact skip and fold contracts, and the generated-QEC hypergraph row records 3.173 thousand detector nodes per second. These rows remain contract-representative, contract-only, or report-only and therefore do not establish Stim-relative speedups. The faithful direct-match generated d25 graphlike row records 120.3 thousand detector nodes per second and `4.324x` pinned Stim, above the `1.25x` parity target. It remains a visible report-only optimization debt; extraction does not weaken its status, threshold policy, or comparator.

The final pure-analysis slice moves ErrorMatcher, matched-error provenance values, compact filter traversal, and their fixed resource admission together. Keeping the matcher beside circuit-to-DEM lowering avoids a facade callback boundary through the candidate-isolation loop, while keeping the provenance values beside the matcher gives direct Stable consumers a complete typed result. The canonical implementation imports only `stab-model` and existing analysis modules; it does not depend on records, mutable execution sessions, the facade, CLI, or ops.

`stab-core` retains explicit `ExplainedError`, `CircuitErrorLocation`, and `CircuitTargetsInsideInstruction` wrappers because their established mutating methods return `CircuitResult`. The wrappers delegate canonical ordering, filling, comparison, and formatting and convert `AnalysisError` losslessly. The four simple provenance values are direct reexports because they have no facade-specific error signature. This preserves source compatibility without duplicating matcher algorithms or making `stab-analysis` depend on the facade.

The focused source commit is `1fa78487`. Canonical matcher, generated-QEC, Pauli-channel, resource, and matched-error suites execute from `stab-analysis`; the mixed folded traversal property remains in core because it composes transforms, sampling, search, SAT, and matching. Matcher filter constants are private to the matcher instead of borrowing DEM-flatten policy names. Stable checks, Stable rustdoc with warnings denied, complete canonical tests, core facade and mixed integration tests, warnings-denied Clippy, benchmark unit tests, and architecture enforcement pass.

Qualification retargets the canonical matcher, matched-error, Python-mined value, repeat-resource, filter-resource, folded-filter, and canonicalization selectors to `stab-analysis`, while root facade APIs remain independently inventoried. The regenerated correctness inventory contains 2,886 upstream cases, 6,782 public API items, 2,108 evidence parents, and digest `7d15a17f698f037e179970d42a020ecb6a4d444d226b6ff5ec84e20a4dc47be1`. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `945bfb72bd431a90979f55578f9fe487fba16649521ba5230e119a578a627cbd`; PF4 matcher measurements call the canonical Stable crate directly, while workloads, semantic work, comparator classes, parity thresholds, regression baselines, and waivers remain unchanged.

The qualification ownership commit is `b731a53b`. Clean-revision matcher evidence is under `target/benchmarks/a6-analysis-matcher-folded-b731a53b-{baseline,compare}`, `target/benchmarks/a6-analysis-matcher-flat-b731a53b-{baseline,compare}`, `target/benchmarks/a6-analysis-matcher-nested-b731a53b-{baseline,compare}`, `target/benchmarks/a6-analysis-matcher-logical-b731a53b-{baseline,compare}`, and `target/benchmarks/a6-analysis-matcher-annotation-b731a53b-{baseline,compare}`; every comparison records commit `b731a53b76eab895d82fdb394a29a4c545f6c8e2` with `local_modifications=false`.

The aggregate folded-traversal row records 43.756 microseconds for bounded ErrorMatcher circuit traversal, or 46.81 million expanded instructions per second. The selected flat detector-touching filter repeat records 699.3 billion folded filter keys per second, the nested repeat records 8.145 trillion folded nested filter keys per second, the detectorless logical-observable repeat records 5.481 trillion folded logical filter keys per second, and the annotation-bearing repeat records 4.259 trillion folded annotated filter keys per second. These rows retain their contract-only classification because the harness has no faithful phase-equivalent pinned Stim comparator for the selected typed operations. They establish clean moved-path health without claiming a Stim-relative ratio.

The first engine slice creates a real `stab-engine` package before moving mutable simulator state. It owns the backend-neutral compilation operation and request fingerprint plus execution-side biased bit randomization. This order proves the package, dependency, rustdoc, qualification, and benchmark ownership boundaries with two low-coupling operations before sampler plans and sessions introduce owner-domain compile errors. `stab-core` retains one-line compatibility reexports, so there is no duplicate implementation.

The focused source commit is `d06e1a3d`, and qualification ownership is synchronized at `2e353a2c`. The regenerated correctness inventory contains 2,886 upstream cases, 6,840 public API items, 2,108 evidence parents, and digest `d69fd880593870446f8b63e6cea424579cdafcc4700f72e4617a0f47b02f4381`; one compilation-fingerprint parent moves from planned to implemented. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `0d27ed46bfff8cd4c790fd8f382c531cd2177d25dae590c5b001cf63985d53df`. The sampling-request fingerprint group and direct benchmark callers now name `stab-engine` as owner, while workload descriptors, comparators, thresholds, regression tolerances, baselines, and waivers remain unchanged.

Clean-revision probability evidence is under `target/benchmarks/a6-engine-foundation-probability-2e353a2c-{baseline,compare}`. The comparison records commit `2e353a2ced922075b10f0981efa548abb2043b3f` with `local_modifications=false` and pinned Stim v1.16.0. All seven direct-match probabilities are faster than their Stim filters in this probe; ratios range from `0.437x` to `0.979x`, and the conservative row ratio is `0.979x`. This remains report-only source-current extraction evidence and does not create a new gate or baseline.

The circuit-sampling slice moves compilation, immutable executable plans, mutable random sessions, direct-Z, small-frame, general-frame, deterministic reference-sample, cancellation, progress, poisoning, and typed `MeasurementSink` delivery into `stab-engine`. The engine depends on model, records, algebra, and analysis and imports no facade, codec, filesystem, CLI, or ops API. `stab-core::CompiledSampler`, callback streaming, materialized records, and byte-oriented encoding remain explicit compatibility adapters. A single canonical crate-root engine API avoids duplicating every sampling item under a second public module namespace. The focused source commits are `4cc8f9d7`, `764ecd0a`, and `3c8347dd`.

Qualification ownership is synchronized at clean revision `18f2e5c9`. The regenerated correctness inventory contains 2,886 upstream cases, 7,109 public API items, 2,109 evidence parents, and digest `27f8ec3116961a48fd0279cbe5e347d6846ee647abe0f22945c8ae00566058de`; every canonical engine sampling item has an implemented owner, with 839 implemented, 17 evidence-close, and 1,253 planned parents overall. The performance inventory retains 127 checklist rows, 179 groups, and 167 inherited decisions with digest `d7de5ad220c539df19af6ae030a269e29280ed464f0867602e61e8339aac9545`. The sampler-compile diagnostic and direct M8 callers now name `stab-engine`; workloads, semantic work, comparator classes, parity thresholds, regression tolerances, baselines, and waivers are unchanged.

Clean-revision M8 evidence is under `target/benchmarks/a6-engine-sampling-18f2e5c9-{baseline,compare}`. The comparison records commit `18f2e5c9e349fa99efe40ccb541e7495bb4131d8` with `local_modifications=false`, pinned Stim v1.16.0, one warmup, and three recorded runs across all 20 M8 rows. All six direct-match rows are faster in this probe: conservative row ratios are `0.979x` for biased randomization, `0.177x` for 01 reading, `0.948x` for b8 reading, `0.633x` for r8 reading, `0.588x` for HITS reading, and `0.877x` for DETS reading. The four process-symmetric CLI sampling rows record `0.992x`, `0.997x`, `1.008x`, and `0.994x`, all within the unchanged `1.25x` parity target. Contract-only, partial-match, and report-only rows retain those classifications and make no new direct Stim claim.

The exact engine-owned compile-and-release diagnostic is under `target/benchmarks/qualification/a6-engine-sampler-compile-18f2e5c9`. Its `raw-work-v2` medians are 448.425, 398.663, and 395.910 nanoseconds per work item for small, medium, and large scales. This is an unverified-host Stab-only product diagnostic, so Stim parity and Stab self-regression are not claimed.
