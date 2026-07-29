# ADR 0005: Backend Selection And Nightly Isolation

## Status

Accepted for Stab 0.2 architecture work.

## Context

Portable SIMD is maintainable and performance-portable, but unconditional crate-level Nightly prevents parser, model, record, and scalar-algebra users from using Stable Rust.

A future GPU backend will require a distinct plan and data layout, not merely replacements for current word kernels.

## Decision

No executable product source currently uses `std::simd`; the former ordinary and Clifford implementations were removed during scalar component extraction.

Any restored direct `std::simd` use belongs only to `stab-kernels-simd` and must be differential-tested against the current scalar references.

`stab-kernels-simd` has no Stab dependency and accepts raw word slices and fixed word blocks only.

Model, bits, records, scalar algebra, and pure analysis support Rust 1.97.1.

The complete engine, facade, and CLI retain the pinned Nightly high-performance build.

Scalar behavior is the absence of the additive `portable-simd` feature.

Backend requests occur during compilation through `Auto`, `Scalar`, and `PortableSimd`. Selected-backend values describe registered implementations only.

A4 registers only the scalar implementation: `Auto` selects scalar, explicit scalar succeeds, and explicit portable SIMD returns a typed unavailable-backend diagnostic before lowering.

A6 registers portable SIMD only after `stab-kernels-simd` owns a distinct measured implementation.

Public plans wrap private backend-specific variants.

No dynamic dispatch occurs in hot loops.

No unimplemented GPU capability is advertised.

## Consequences

- Stable consumers can use reusable toolkit components.
- Full Stab performance retains portable SIMD.
- A later device backend must prove a real plan and batch contract before becoming public.
