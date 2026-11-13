# ADR 0008: Unconditional Leaf Kernels On Fearless SIMD

## Status

Accepted for Stab 0.2. Supersedes [ADR 0005](adr-0005-backends-and-nightly.md) for leaf-kernel selection and the toolchain boundary, and amends the kernel edges of [ADR 0007](adr-0007-product-dependency-graph.md).

## Context

ADR 0005 isolated Nightly `std::simd` kernels in an optional `stab-kernels-simd` crate behind an additive `portable-simd` Cargo feature, so Stable component users compiled duplicate scalar paths by default.

The [fearless SIMD and Stable Rust plan](../plans/fearless-simd-stable-rust-plan.md) ported the kernels to [`fearless_simd`](https://github.com/linebender/fearless_simd), which builds on Stable Rust through a safe API. Its S2 checkpoint measured the port as performance-neutral against both the `std::simd` build and the scalar build on the AArch64 development host.

Keeping two production implementations of each kernel and a build-time switch between them doubles the code that must stay semantically identical without any measured benefit.

## Decision

`stab-kernels-simd` is a mandatory, Stable, unconditional dependency of `stab-bits` and `stab-algebra`. There is no Cargo feature that selects between SIMD and scalar kernels.

Each kernel has exactly one production implementation, built on `fearless_simd` and exposed as slice-level functions. Scalar formulas remain in production only where they are not a kernel replacement: kernel tails inside `stab-kernels-simd`, `BitBlock`'s single four-word operations, `masked_xor_assign_words`, and other word loops that never went through the kernel crate. Scalar reimplementations of the kernels survive only as test references.

Instruction-set selection happens once per slice-level call, outside every word loop. It is static on AArch64 (NEON) and a runtime level check on x86-64 (SSE2 through AVX-512).

`stab-kernels-simd` is the only crate allowed to depend on `fearless_simd` or `fearless_simd_macros`. It keeps `unsafe_code = "deny"`, takes no Stab dependency, owns no Stab types, logical tails, allocation, or sampling backend policy, and accepts raw word slices only.

The following ADR 0005 rules carry over unchanged:

- No placeholder backend enum, registry, or unavailable choice is published.
- No dynamic dispatch occurs inside hot loops.
- Leaf kernels are not a sampling backend. A future packed-frame or device implementation may introduce backend selection only after the engine owns two real executable plans, distinct plan fingerprints, semantic equivalence for each affected plan family, and phase-specific performance evidence.
- No unimplemented GPU capability is advertised.

The workspace builds with one Stable toolchain, Rust 1.100.0, pinned in `rust-toolchain.toml`. Every product crate, including `stab-core`, `stab-cli`, and `stab-kernels-simd`, declares Rust 1.100 as its minimum supported version, and there is no Nightly package class.

## Consequences

- Every consumer of `stab-bits` or `stab-algebra` compiles the same kernels, so there is no scalar-versus-SIMD build matrix to test or benchmark.
- Library consumers who previously used the scalar default now resolve `fearless_simd` and `fearless_simd_macros` transitively.
- x86-64 consumers pay one runtime level check per kernel call; the cost of that check on short slices has not been measured on a controlled x86-64 host.
- Per-level differential tests in `stab-kernels-simd` are the protection for instruction-set paths that the AArch64 evidence host never executes.

## Enforcement

`just architecture::check` rejects any resolved dependency on `fearless_simd` or `fearless_simd_macros` from a package other than `stab-kernels-simd`, requires the exact Rust 1.100 minimum in every product crate including `stab-kernels-simd`, `stab-core`, and `stab-cli`, and rejects unstable `feature(...)` gates in every product crate.

`just architecture::consumer-check` requires exactly one resolved `stab-kernels-simd` package in the component-only, facade, and mixed direct-component consumer fixtures.
