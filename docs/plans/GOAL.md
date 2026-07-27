# Goal: Finish Agent-Native Milestone A2

## Objective

Finish milestone A2 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) without weakening implemented Stim v1.16.0 behavior, resource safety, or the existing `1.25x` parity contracts.

Do not begin A3 crate and batch work until A2 passes its milestone audit.

## Sources Of Truth

- Active architecture plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md)
- Current implementation record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Architecture contracts: [../architecture/README.md](../architecture/README.md)
- Generated qualification state: [../qualification-status.md](../qualification-status.md)
- Correctness and performance contracts: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md) and [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Planning lessons: [lessons-learned.md](lessons-learned.md)

Stop and repair the owning source when these documents, generated inventories, or code disagree.

## Current State

- A0 and A1 are complete.
- A2 is active at implementation checkpoint `688495fd`.
- Discovery, model and request fingerprints, bounded `inspect` and `plan sample`, folded herald counting, codec-owned output estimates, and four Stab-only product diagnostics are implemented.
- Correctness inventory: `3c08ac35fe7379f427d5512f98033353844f25053a16093a1e0a61f8085cf976`.
- Performance inventory: `4902a52d00d291d6e2b8447c83262e9087bdc246de3ba3befc18ed1abcc09da8`.
- Formal completion for these inventories has not started.

## Remaining A2 Work

1. Give circuit and DEM parse failures stable domain codes, exact byte spans, typed context, and non-lossy facade conversion.
2. Prove LF, CRLF, UTF-8 tag, malformed-byte, nesting, numeric, and EOF locations without changing accepted grammar, human messages, precedence, or exit status.
3. Inventory remaining configurable safety constants by operation.
4. Introduce only justified operation-owned `CompileLimits`, `SamplingLimits`, `MaterializationLimits`, and `SearchLimits`; keep semantic and platform invariants private and non-overridable.
5. Preserve every default accepted maximum and first rejection, reject invalid overrides before allocation, RNG advancement, output creation, or expensive work, and add cheap typed estimates only where defensible.
6. Propagate new diagnostics through human and JSON CLI paths and regenerate exact correctness ownership.
7. Run focused allocation and timing probes for touched hot paths. Reuse existing comparable benchmarks; add no mixed-phase or speculative product rows.
8. Run `milestone-audit` and `full-code-review`, fix confirmed findings, and record genuine under-specification only in the gap log.

## Guardrails

- Human CLI behavior remains the default; JSON is additive.
- The Stim circuit and DEM dialects stay closed.
- `PlanFingerprint`, backend selection, sessions, and execution batching remain A4 work.
- The four A2 diagnostics remain Stab-only and report-only. Do not infer Stim parity or incremental request cost from independent medians.
- Product code never depends on qualification code.
- No policy override may bypass a semantic invariant or current recursive safety envelope.
- Dirty or unverified-host timing remains diagnostic and non-promotable.

## Immediate Checks

```text
cargo test -p stab-core diagnostics --quiet
cargo test -p stab-core parse_limits --quiet
cargo test -p stab-cli error_format --quiet
cargo test -p stab-cli agent --quiet
just architecture::check
just qualification::correctness-regenerate --check
just bench::qualification-regenerate --check
just qualification::status --check
```

Before an A2 completion claim, run workspace format, Clippy, tests, rustdoc, oracle result-format checks, benchmark smoke, pre-commit, milestone audit, and full code review from a clean committed revision.
