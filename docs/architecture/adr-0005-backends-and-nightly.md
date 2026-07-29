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

Model, bits, records, scalar algebra, and pure analysis support Rust 1.97.1.

The complete engine, facade, and CLI retain the pinned Nightly high-performance build.

Scalar behavior is the absence of the additive `portable-simd` feature.

Backend requests occur during compilation through `Auto`, `Scalar`, and `PortableSimd`. Selected-backend values describe registered implementations only.

A4 registers only the scalar implementation: `Auto` selects scalar, explicit scalar succeeds, and explicit portable SIMD returns a typed unavailable-backend diagnostic before lowering.

A6 does not register portable SIMD as a sampling backend. Build-time bit and algebra acceleration cannot represent two runtime backends after Cargo feature unification, and the current sampling plans do not execute through these raw kernels.

A later packed-frame milestone may register portable SIMD only after the engine owns a distinct executable plan, a distinct plan fingerprint, semantic equivalence for each affected plan family, and phase-specific performance evidence.

Public plans wrap private backend-specific variants.

No dynamic dispatch occurs in hot loops.

No unimplemented GPU capability is advertised.

## Consequences

- Stable consumers can use reusable toolkit components.
- Nightly consumers can opt into measured portable bit and algebra kernels without changing sampling capability claims.
- A later device backend must prove a real plan and batch contract before becoming public.
