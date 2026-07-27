# Goal: Extract Stable Bit And Record Crates

## Objective

Finish milestone A3 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) by physically extracting `stab-bits` and `stab-records`, introducing typed shot-major and bit-plane batches, and preserving exact Stim v1.16.0 result-format behavior.

The extraction must leave Stable Rust users able to parse, transform, and write result records without compiling `stab-core`, the CLI, or Nightly portable-SIMD code.

## Why This Milestone

A2 established typed diagnostics and resource boundaries while one crate still owned the implementation. A3 is the first physical product split because packed storage and result codecs are leaf domains with a clear dependency direction: `stab-records -> stab-bits`. Extracting these leaves first tests the component architecture without forcing the simulator, circuit model, and analysis graph apart prematurely.

## Sources Of Truth

- Active plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), milestone A3
- Append-only record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Product graph and contracts: [../architecture/README.md](../architecture/README.md)
- Result-format oracle: [../../oracle/result-format-corpus.json](../../oracle/result-format-corpus.json)
- Generated status: [../qualification-status.md](../qualification-status.md)

Stop and repair the owning source when code, tests, generated inventories, benchmarks, or these contracts disagree.

## Current State

- A0, A1, and A2 are complete.
- A2 closed at clean source revision `7b6c592b08f6a24d31a0673588dce7525b1c02c9`.
- The workspace still has no `stab-bits` or `stab-records` package.
- Existing bit storage, record layouts, strict text lexers, typed DETS parsing, streaming visitors, and writers remain inside `stab-core`.

## Execution Sequence

1. Inventory the exact modules, public items, tests, feature flags, and dependency edges that belong to bits and records. Resolve any cycle before moving files.
2. Create publishable Stable Rust 1.97.1 `stab-bits` with checked packed storage, borrowed views, layout primitives, scalar kernels, and no dependency on Stab product crates.
3. Create publishable Stable Rust 1.97.1 `stab-records` depending only on `stab-bits` and ordinary Stable dependencies.
4. Move strict `01`, HITS, DETS, `b8`, `r8`, and PTB64 codecs plus typed layouts and visitors into `stab-records`; keep compatibility re-exports in `stab-core`.
5. Add owned and borrowed shot-major and bit-plane batches, separate detector and observable planes, and bounded conversion between layouts.
6. Make writers consume typed records or batches. Keep record-at-a-time callbacks as bounded adapters with first-error and cancellation guarantees.
7. Move corpus ownership tests to the extracted crates and run every checked case through direct component APIs, `stab-core` compatibility paths, the CLI, and pinned Stim.
8. Add property tests for layout conversion, tail bits, zero widths, namespaces, duplicates, PTB64 groups, bounded allocation, retained capacity, and cancellation.
9. Bind shot-major writing, bit-plane writing, transpose, DETS parsing, representative format conversion, and reusable-codec allocation to focused benchmarks. Compare pre-extraction and post-extraction evidence before making a performance claim.
10. Regenerate architecture and qualification inventories, run milestone-audit and full-code-review, fix confirmed findings, update the progress report, and commit each coherent extraction or contract change separately.

## Guardrails

- Preserve exact accepted bytes, rejected grammar, canonical output, ordering, and error class for implemented Stim formats.
- Do not expose portable SIMD from either Stable crate. A later `stab-kernels-simd` crate owns Nightly kernels.
- Do not let `stab-records` depend on `stab-core`, CLI, ops, circuit models, simulators, or qualification code.
- Keep detector and observable data distinct in typed APIs; combine them only at explicit codec boundaries.
- Bound conversion by declared batch dimensions and checked arithmetic before allocation.
- Preserve existing `stab-core` source compatibility where practical; document and qualify any unavoidable public API change.
- Do not start A4 sampling-plan or session work during A3.

## Done Criteria

- Cargo metadata shows physical `stab-bits` and `stab-records` packages with only permitted edges.
- Both crates build and test on Rust 1.97.1.
- Stable callers can parse and convert all implemented result formats without `stab-core` or Nightly.
- All 62 checked corpus cases pass through direct extracted APIs and the live pinned-Stim differential.
- Typed batch round trips, bounded allocation, cancellation, first-error behavior, and exact Stim bytes have direct tests.
- Focused benchmark evidence shows no unexplained material regression in affected codecs.
- Architecture checks, generated inventories, milestone-audit, full-code-review, and standard workspace verification have no open A3 blocker.

## Required Checks

```text
cargo +1.97.1 check -p stab-bits -p stab-records
cargo +1.97.1 test -p stab-bits -p stab-records
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
just architecture::check
just oracle::result-formats --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```
