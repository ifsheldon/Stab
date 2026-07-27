# Agent-Native Modular QEC Progress Report

Current as of 2026-07-27.

## Status

- A0 architecture contract and baseline: complete.
- A1 logical ownership and dependency enforcement: complete.
- A2 diagnostics, resources, fingerprints, and capabilities: active at committed checkpoint `3454722`.
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
