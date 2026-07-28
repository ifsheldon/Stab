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
