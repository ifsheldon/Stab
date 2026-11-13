# Fearless SIMD And Stable Rust Plan

Status: Approved on 2026-11-14 and active; S1 is in progress. This plan is separate from the [Stim core parity plan](stim-core-parity-and-lean-evidence-plan.md) and changes no parity scope.

## Summary

Replace the Nightly `std::simd` word kernels with [`fearless_simd`](https://github.com/linebender/fearless_simd), then move the whole workspace from the pinned Nightly to Stable Rust 1.100.0 and declare 1.100 as the minimum supported Rust version of every product crate.

The `portable-simd` Cargo feature survives only long enough to measure the port. Once the measurements show no regression, the feature and the duplicate production scalar paths are deleted, and the `fearless_simd` kernels become the single unconditional implementation.

## Rationale

- The only unstable Rust feature in the workspace is `portable_simd`, and it is confined to the 166-line `crates/stab-kernels-simd/src/lib.rs`. Every other crate already checks on Stable 1.100.0.
- The two kernels use only `&`, `|`, `^`, and `!` over `u64` lanes, which `fearless_simd` provides on Stable through a safe API.
- A Nightly pin predating three point releases that fix miscompilations is a standing risk; see `docs/rust-releases-1.96-1.100.md` while that note exists.
- One toolchain and one kernel implementation remove the Nightly and Stable contract split, the opt-in feature graph, and most consumer-fixture variation from the architecture checker.

## Decisions

| Topic | Decision |
| --- | --- |
| SIMD library | `fearless_simd` 1.2.0 with the `#[simd]` attribute from `fearless_simd_macros` 0.1.0; re-check crates.io for the latest stable versions when S1 starts. |
| Minimum Rust | 1.100 for every product crate, including `stab-core`, `stab-cli`, and `stab-kernels-simd`. |
| Toolchain pin | `rust-toolchain.toml` moves to `1.100.0`; no Nightly remains. |
| Feature flag | `portable-simd` is kept through S2 for measurement only, then deleted in S3. |
| End state | SIMD kernels are always on; scalar formulas survive only as kernel tails, the single-block XOR, and test references. |
| Kernel crate | `stab-kernels-simd` stays a separate product crate and becomes the only crate allowed to depend on `fearless_simd`. |
| Order | Port on the pinned Nightly first, then move the toolchain, so the library swap and the compiler change are measured separately. |

## Design

### Kernel API

`fearless_simd` selects the instruction set at runtime on x86 (SSE2, SSE4.2, AVX2, Ice Lake AVX-512) and statically on AArch64 (NEON). Dispatch must therefore happen once per slice operation, never once per four-word block.

- `stab-kernels-simd` exports slice-level `xor_assign_words` and `clifford_right_multiply_words` and owns chunking, tails, and non-identity counting. The per-block functions, `WordBlock`, `CliffordWordBlock`, and `WORD_LANES` leave its public API.
- Kernels are generic over `S: Simd`, annotated with `#[simd]`, and written against the native-width `S::u64s` vector so one source serves 128-bit, 256-bit, and 512-bit hardware.
- One generic Clifford product formula over any `BitAnd + BitOr + BitXor + Not` type serves both the vector body and the scalar tail, so the two cannot drift.
- Plane-length agreement becomes a checked property of the kernel's input types instead of a `debug_assert!` at the call site. S1 settles the exact shape and confirms that it adds no per-word cost.
- `BitBlock`'s single four-word XOR uses the scalar formula directly; one block does not justify a dispatch.
- `stab-kernels-simd` keeps `unsafe_code = "deny"`, takes no Stab dependency, and accepts raw word slices only.

### Removed In The End State

- `crates/stab-bits/src/portable.rs` and the `selected` module indirection in `stab-bits`.
- `scalar_clifford_right_multiply_words`, `portable_clifford_right_multiply_words`, and the feature switch in `crates/stab-algebra/src/kernels.rs`.
- The `portable-simd` feature in `stab-bits`, `stab-algebra`, `stab-engine`, `stab-core`, `stab-cli`, `stab-bench`, and `stab-oracle`, with its uses in `justfiles/bench.just`, CI, and `ops/bench/src/e2e/execution.rs`.
- The scalar and Nightly consumer distinctions under `test-support/consumers/`.

### Architecture Policy

- [ADR 0005](../architecture/adr-0005-backends-and-nightly.md) is superseded by a new ADR that records one Stable toolchain, unconditional leaf kernels, and per-call instruction-set selection outside word loops. Its rules against placeholder backends, hot-loop dynamic dispatch, and unproven sampling backends carry over unchanged.
- `just architecture::check` replaces the Nightly contract, the `std::simd` source scan, and the Stable-default-reaches-Nightly rule with three rules: no unstable `feature(...)` gate in any product crate, a `fearless_simd` or `fearless_simd_macros` dependency edge only from `stab-kernels-simd`, and the exact 1.100 minimum in every product manifest.
- `just architecture::consumer-check` keeps one Stable component consumer, one facade consumer, and one mixed direct-component consumer, and asserts exactly one resolved kernel package in each.

## Feasibility Evidence

Recorded on 2026-11-13 on the AArch64 development host. None of this is performance evidence.

- A scratch crate implementing both kernels with `S::u64s`, `#[simd]`, and `dispatch!` compiles under `unsafe_code = "deny"` and matches a scalar reference for 0 through 39 words on Stable 1.100.0. The manual `#[inline(always)]` variant also checks on 1.97.1.
- `cargo +stable check --workspace --exclude stab-kernels-simd --all-targets --locked` passes on 1.100.0 with only Cargo manifest warnings that `package.homepage` duplicates `package.repository`.
- `cargo +stable clippy` with warnings denied fails on `chunks_exact_to_as_chunks`. Seven errors appeared before compilation stopped, there are 32 `chunks_exact` call sites, and further lints may be hidden behind those failures.
- `stab-core` and `stab-cli` already check on 1.97.1 with scalar defaults.
- `stab-kernels-simd` is unpublished, so its API can change without a migration path.
- Not verified: any x86 code path, any timing, and whether Stable `rustfmt` formats the workspace identically to the pinned Nightly.

## Milestone S1: Port The Kernels On The Pinned Nightly

### Tasks

1. Add `fearless_simd` and `fearless_simd_macros` to `[workspace.dependencies]` and to `stab-kernels-simd`.
2. Rewrite `stab-kernels-simd` to the slice-level API in [Design](#design) and remove `#![feature(portable_simd)]`.
3. Point `stab-bits` and `stab-algebra` at the slice-level kernels, still behind `portable-simd`, and leave the scalar paths untouched.
4. Update the crate description, `docs/MIGRATING-0.2.md`, and `README.crates.md` where they call the crate dependency-free or Nightly-only.

### Tests

- Differential tests in `stab-kernels-simd` run the generic kernels against an independent scalar reference at every instruction-set level the test host can construct, plus the forced scalar fallback, across lengths that cover empty input, tails, and several full vectors at 128-bit, 256-bit, and 512-bit widths.
- The existing `portable-simd` tests in `stab-bits` and `stab-algebra` keep passing, including the zero-allocation Clifford test.
- `just oracle::parity-run` passes with the feature on.

### Done Criteria

- No `std::simd`, `core::simd`, or `feature(...)` gate remains in the workspace.
- The pinned Nightly build, strict Clippy, and the existing architecture checks pass with the feature both on and off.

## Milestone S2: Measure The Port

### Tasks

1. On the controlled AArch64 host and the pinned Nightly, measure the last `std::simd` revision and the S1 revision with `portable-simd` on, using the existing E2E bundle and baseline-candidate tooling.
2. Measure a scalar build of the S1 revision against its SIMD build as a one-off diagnostic, because S3 makes SIMD the default for library consumers who are scalar today.
3. Run a short-slice diagnostic on an x86-64 machine comparing the dispatched kernels with scalar for one-word through eight-word inputs.
4. Record the per-case paired ratios in this plan's S2 checkpoint.

### Benchmarks

- No release case, diagnostic row, threshold, or workload changes. The comparison reuses the 25-case release matrix left by P10, which has no accepted baselines.
- The S1 revision must stay within the `1.15x` self-regression ratio of the `std::simd` revision in every case, measured as a paired comparison of the two revisions on the same host and toolchain.
- Five cases already exceed the `1.25x` Stim ceiling before this plan for reasons the engine plan owns: `sample-surface.d25-b8`, `sample-dem.surface-d11-side-outputs`, `detect-observables.d11-sparse-hits`, `detect-observables.color-d9-b8`, and `analyze-decompose.d25`. They must not get worse here, and they are not this plan's to fix.
- Full and soak tiers need a clean committed revision, so until S1 is committed the comparison uses the smoke tier and direct paired timing of the two builds.
- x86-64 results are shared-host diagnostics and promote no performance claim.

### Done Criteria

- The ratios are recorded and the project owner confirms that performance is not affected.
- If the x86-64 diagnostic shows a short-slice regression, S3 adds a fixed short-slice scalar path inside the kernel crate instead of keeping a feature flag.
- If the AArch64 comparison fails, this plan stops before S3 and the cause is investigated; no gate is relaxed.

## Milestone S3: Delete The Flag And The Scalar Production Paths

### Tasks

1. Make `stab-kernels-simd` a mandatory dependency of `stab-bits` and `stab-algebra`, and delete everything listed under [Removed In The End State](#removed-in-the-end-state).
2. Keep the scalar formulas needed by tails, the single-block XOR, and `masked_xor_assign_words`; move the remaining scalar kernels into tests as references.
3. Write the superseding ADR and update `docs/architecture/README.md`, `docs/MIGRATING-0.2.md`, and `README.crates.md`.
4. Rewrite the feature-graph rules and consumer fixtures as described under [Architecture Policy](#architecture-policy), and update the package lists in `ops/oracle/src/parity.rs` and the publish order in `ops/release` if the mandatory edge changes them.

### Tests

- The S1 differential tests remain the kernel owners; the former feature-gated tests in `stab-bits` and `stab-algebra` run unconditionally.
- Architecture policy tests cover a forbidden `fearless_simd` edge from a non-kernel crate and a missing kernel package in a consumer graph.
- `just oracle::parity-check`, `just oracle::parity-run`, and `just oracle::parity-render --check` pass.

### Done Criteria

- `portable-simd` appears nowhere in manifests, source, `just` recipes, CI, or active documentation.
- Each kernel has one production implementation.

## Milestone S4: Move To Stable Rust 1.100

### Tasks

1. Set `rust-toolchain.toml` to `1.100.0` and declare `rust-version = "1.100"` in every product crate, the Stable test-support crates, and the consumer fixtures.
2. Fix Stable Clippy findings with behavior preserved, starting with the `chunks_exact` sites, where each conversion must keep its remainder handling.
3. Resolve the `package.homepage` manifest warnings.
4. Collapse the separate `cargo +1.97.1` CI job and the portable-SIMD CI steps into the single pinned toolchain, and update the release workflow.
5. Replace the Nightly and Stable package contracts and version constants in `ops/architecture/src/policy.rs`, and retire the portable-SIMD source scan.
6. Update the root `AGENTS.md` Rust section, `CONTRIBUTE.md`, `docs/RELEASING.md`, `docs/architecture/README.md`, and `README.crates.md` for the single Stable toolchain.

### Tests

- `cargo fmt --check`, `cargo clippy --workspace --all-targets` with warnings denied, and `cargo test --workspace` pass on 1.100.0.
- `just architecture::check`, `just architecture::consumer-check`, and `just architecture::docs-check` pass.
- Architecture policy tests cover a product manifest with a missing or wrong minimum Rust version and a product source containing a `feature(...)` gate.

### Done Criteria

- No Nightly toolchain is installed or referenced by the workspace, CI, or active documentation.
- Adopting 1.98 through 1.100 language or library features is out of scope here and needs its own measured change where a benchmarked path is involved.

## Milestone S5: Controlled Evidence

### Tasks

1. Produce a controlled AArch64 full-and-soak pair on Stable 1.100.0 that records the post-port state on the new toolchain.
2. Keep that pair as the reference the engine plan measures its milestones against.
3. Leave baseline seeding and `just bench::e2e-release-check` to the engine plan's closing evidence, because the five cases listed under S2 cannot pass the Stim ceiling until that plan lands.

### Done Criteria

- The pair passes correctness, memory, and offline replay, and no case is slower relative to Stim than it was before this plan.
- No gate is relaxed and no failing case is reported as passing.

## Sequencing

P10 phases 0 through 5 finished at `a3260617` on 2026-11-14, so this plan can start. It runs before the [engine speed and architecture plan](engine-speed-and-architecture-plan.md), whose milestones are developed and measured on the toolchain and kernels this plan leaves behind.

P10's formal evidence phase has not run and is blocked on performance gaps that the engine plan owns. The order is therefore this plan, then the engine plan, then one formal full-and-soak pair that closes both plans and P10.

## Risks

- Performance on AArch64 is expected to be neutral because both libraries lower four `u64` lanes to two 128-bit NEON operations, but this is an expectation until S2 measures it.
- CI runs on x86-64 and can execute AVX2 or AVX-512 paths that the AArch64 evidence host never runs; the per-level differential tests are the protection.
- Always-on SIMD adds a runtime level check per kernel call on x86-64, and there is no controlled x86-64 host to qualify it formally.
- Rust 1.99 moved to LLVM 23, so S4 can shift timings independently of the port; keeping S2 on the pinned Nightly isolates that effect.
- `fearless_simd_macros` is a 0.1 release versioned separately from the 1.x core crate.
- Another change stream is active in the repository; S1 touches only the kernel crate and its two call sites to limit conflicts.
