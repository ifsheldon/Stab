# ADR 0005: Backend Selection And Nightly Isolation

## Status

Accepted for Stab 0.2 architecture work.

## Context

Portable SIMD is maintainable and performance-portable, but unconditional crate-level Nightly prevents parser, model, record, and scalar-algebra users from using Stable Rust.

A future GPU backend will require a distinct plan and data layout, not merely replacements for current word kernels.

## Decision

The optional `stab-kernels-simd` crate owns executable four-word XOR and Clifford-composition kernels. Stable component defaults do not compile it.

Any restored direct `std::simd` use belongs only to `stab-kernels-simd` and must be differential-tested against the current scalar references.

`stab-kernels-simd` has no Stab dependency and accepts raw word slices and fixed word blocks only.

Model, bits, records, scalar algebra, pure analysis, and the scalar engine support Rust 1.97.1.

The facade and CLI retain the pinned Nightly workspace build. The engine remains Stable-compatible while its only registered backend is scalar and it has no dependency on the raw SIMD kernel crate.

Making the default engine Nightly-only would prevent Stable decoder and orchestration crates from composing public plans, sessions, and typed sinks without providing a corresponding execution benefit. A future backend that needs Nightly must use an explicit feature or a separate implementation crate and prove a distinct executable plan before changing this boundary.

Scalar behavior is the absence of the additive `portable-simd` feature.

Sampling compilation has no backend request. The compiler constructs the sole scalar implementation, while `SamplingPlan::backend` and `PlanFingerprint::backend` report that actual executable identity.

A6 does not register portable SIMD as a sampling backend. Build-time bit and algebra acceleration cannot represent two runtime backends after Cargo feature unification, and the current sampling plans do not execute through these raw kernels.

A future packed-frame or device implementation may introduce selection only after the engine owns two real executable plans, distinct plan fingerprints, semantic equivalence for each affected plan family, and phase-specific performance evidence. No placeholder enum, registry, or unavailable choice is published beforehand.

Public plans wrap private backend-specific variants.

No dynamic dispatch occurs in hot loops.

No unimplemented GPU capability is advertised.

## Consequences

- Stable consumers can use reusable toolkit components.
- Stable consumers can compile and execute the scalar plan/session pipeline without importing the facade or CLI.
- Nightly consumers can opt into measured portable bit and algebra kernels without changing sampling capability claims.
- A later device backend must prove a real plan and batch contract before becoming public.
