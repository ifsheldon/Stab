# Fearless SIMD And Stable Rust Plan

Status: Complete on 2026-11-14 through S4, with the S5 reference recorded from the smoke tier. Formal evidence is deferred to the engine plan's closing run. This plan is separate from the [Stim core parity plan](stim-core-parity-and-lean-evidence-plan.md) and changes no parity scope.

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

### Checkpoint (2026-11-14)

S1 landed with one deviation from the design: the main loops use fixed `u64x8` blocks for XOR and `u64x4` blocks for Clifford products on 128-bit levels, then finish with the native `S::u64s` vector and scalar words. Plain native-width loops were 8 to 11 percent slower at two sizes on NEON because each store landed before the next load.

Measured on the AArch64 development host with the pinned Nightly, best of three alternating runs on one pinned core, in nanoseconds per call through the public `BitVec::xor_assign` and `CliffordString::right_multiply_in_place`:

| Kernel and size | `std::simd` | `fearless_simd` | Scalar | Port / `std::simd` |
| --- | ---: | ---: | ---: | ---: |
| XOR, 64 bits | 4.29 | 4.29 | 4.29 | 1.000 |
| XOR, 256 bits | 8.65 | 6.79 | 8.60 | 0.785 |
| XOR, 1024 bits | 8.57 | 8.65 | 8.57 | 1.009 |
| XOR, 4096 bits | 14.46 | 14.77 | 14.79 | 1.021 |
| XOR, 65,536 bits | 199.91 | 197.12 | 199.88 | 0.986 |
| XOR, 1,048,576 bits | 5219 | 5259 | 5219 | 1.008 |
| XOR, 16,777,216 bits | 85,801 | 84,610 | 85,999 | 0.986 |
| Clifford, 64 qubits | 32.69 | 24.47 | 26.14 | 0.749 |
| Clifford, 1024 qubits | 99.28 | 95.42 | 114.83 | 0.961 |
| Clifford, 65,536 qubits | 5119 | 5061 | 5093 | 0.989 |
| Clifford, 1,048,576 qubits | 89,331 | 90,888 | 83,469 | 1.017 |

End-to-end, best of five interleaved CLI runs on one pinned core, in milliseconds:

| Workload | `std::simd` build | `fearless_simd` build | Scalar build | Port / `std::simd` |
| --- | ---: | ---: | ---: | ---: |
| `sample`, d=11, 300,000 shots | 236.0 | 236.8 | 240.2 | 1.00 |
| `sample`, d=25, 25,000 shots | 1965.5 | 1961.4 | 1968.1 | 1.00 |
| `sample`, d=7, 1000 rounds, 100,000 shots | 2620.7 | 2648.2 | 2651.9 | 1.01 |
| `detect`, d=11, 350,000 shots, `b8` | 329.9 | 333.2 | 331.4 | 1.01 |
| `detect`, d=11, 300,000 shots, `hits` | 1269.0 | 1272.8 | 1269.8 | 1.00 |
| `m2d`, d=11, 200,000 records | 123.5 | 123.4 | 123.9 | 1.00 |
| `analyze_errors --decompose_errors`, d=21 | 887.4 | 888.1 | 884.6 | 1.00 |
| `analyze_errors --fold_loops`, d=21 | 192.2 | 193.6 | 192.9 | 1.01 |
| `sample_dem`, d=11, 100,000 shots | 317.2 | 317.9 | 317.3 | 1.00 |
| `gen`, d=51 | 16.2 | 16.4 | 16.5 | 1.02 |
| `convert`, `b8` to `01`, 200,000 records | 94.9 | 95.3 | 94.8 | 1.00 |

Conclusions:

- The port does not change performance: every row is within two percent of the `std::simd` build, and the two small-size rows that moved are faster.
- On this host the scalar build is also within two percent of both SIMD builds, because the compiler already vectorizes the scalar loops; the kernels are on no end-to-end hot path.
- The x86-64 short-slice diagnostic was not run because no x86-64 host is available, so the cost of per-call level selection on x86-64 remains unmeasured.
- Under the project owner's standing decision that an unaffected result means the legacy code is deleted, S3 proceeds.

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

### Checkpoint (2026-11-14)

S4 landed at `f285fcb4`. Stable 1.100 Clippy fired `chunks_exact_to_as_chunks` at 22 sites and `manual_slice_fill` at one; each was converted with the same remainder handling and no lint was silenced. No test expectation changed on the new compiler.

Paired end-to-end timings of the Nightly-built and Stable-built CLI, best of five interleaved runs on one pinned core, in milliseconds:

| Workload | Nightly | Stable 1.100 | Stable / Nightly |
| --- | ---: | ---: | ---: |
| `sample`, d=11, 300,000 shots | 236.7 | 238.8 | 1.01 |
| `sample`, d=25, 25,000 shots | 1959.7 | 2006.4 | 1.02 |
| `sample`, d=7, 1000 rounds, 100,000 shots | 2637.6 | 2666.3 | 1.01 |
| `detect`, d=11, 350,000 shots, `b8` | 328.6 | 333.8 | 1.02 |
| `detect`, d=11, 300,000 shots, `hits` | 1266.4 | 1277.5 | 1.01 |
| `detect`, color code d=9, 200,000 shots | 80.1 | 81.3 | 1.02 |
| `m2d`, d=11, 200,000 records | 120.2 | 119.6 | 1.00 |
| `analyze_errors --decompose_errors`, d=21 | 872.1 | 827.8 | 0.95 |
| `analyze_errors --fold_loops`, d=21 | 191.6 | 182.6 | 0.95 |
| `sample_dem`, d=11, 100,000 shots | 317.1 | 311.1 | 0.98 |
| `CZ` and `S` layer circuit, 20,000 shots | 2318.6 | 2146.2 | 0.93 |

The compiler change is neutral for sampling and detection and slightly faster for analysis.

## Milestone S5: Controlled Evidence

### Tasks

1. Produce a controlled AArch64 full-and-soak pair on Stable 1.100.0 that records the post-port state on the new toolchain.
2. Keep that pair as the reference the engine plan measures its milestones against.
3. Leave baseline seeding and `just bench::e2e-release-check` to the engine plan's closing evidence, because the five cases listed under S2 cannot pass the Stim ceiling until that plan lands.

### Done Criteria

- The pair passes correctness, memory, and offline replay, and no case is slower relative to Stim than it was before this plan.
- No gate is relaxed and no failing case is reported as passing.

### Checkpoint (2026-11-14)

A full-tier run on `1bfbcef8` was refused because swap pages were read during timing, so the reference recorded here is the smoke tier on the same clean revision, bundle `target/benchmarks/2026-11-14-s5-stable-1.100-smoke`. Smoke timing is diagnostic and promotes no claim. All 25 cases pass correctness and memory.

| Case | Stab/Stim |
| --- | ---: |
| `generate.surface-large` | 0.739 |
| `generate.color-large` | 0.771 |
| `convert.sparse-r8-wide` | 0.806 |
| `convert.dense-b8-to-01` | 0.176 |
| `convert.typed-dets-wide` | 0.867 |
| `sample-surface.medium` | 0.916 |
| `sample-surface.large` | 0.909 |
| `sample-surface.d25-b8` | 6.962 |
| `sample-folded-ptb64.large` | 1.089 |
| `detect-observables.medium` | 1.089 |
| `detect-observables.large` | 0.997 |
| `detect-observables.d11-b8` | 1.102 |
| `detect-observables.d11-sparse-hits` | 1.540 |
| `detect-observables.color-d9-b8` | 1.460 |
| `m2d.sweep-medium` | 1.053 |
| `m2d.sweep-large` | 1.060 |
| `m2d.surface-d11` | 0.545 |
| `analyze-folded.large` | 1.131 |
| `analyze-decompose.d15` | 1.167 |
| `analyze-decompose.d25` | 1.228 |
| `sample-dem.medium` | 1.086 |
| `sample-dem.large` | 1.006 |
| `sample-dem.surface-d11-side-outputs` | 4.366 |
| `qec-cli-pipeline.large` | 1.003 |
| `qec-rust-pipeline.large` | n/a |

The five cases above the `1.25x` ceiling match the list recorded under S2 within smoke noise, and none belongs to this plan. A formal full-and-soak pair needs swap disabled on the host and is left to the engine plan's closing evidence.

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
