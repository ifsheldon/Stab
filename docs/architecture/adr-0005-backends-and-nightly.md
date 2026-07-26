# ADR 0005: Backend Selection And Nightly Isolation

## Status

Accepted for Stab 0.2 architecture work.

## Context

Portable SIMD is maintainable and performance-portable, but unconditional crate-level Nightly prevents parser, model, record, and scalar-algebra users from using Stable Rust.

A future GPU backend will require a distinct plan and data layout, not merely replacements for current word kernels.

## Decision

Every direct `std::simd` use moves from the current ordinary and Clifford bit kernels to `stab-kernels-simd`.

`stab-kernels-simd` has no Stab dependency and accepts raw word slices and fixed word blocks only.

Model, bits, records, scalar algebra, and pure analysis support Rust 1.97.1.

The complete engine, facade, and CLI retain the pinned Nightly high-performance build.

Scalar behavior is the absence of the additive `portable-simd` feature.

Backend selection occurs during compilation and supports only `Auto`, `Scalar`, and `PortableSimd`.

Public plans wrap private backend-specific variants.

No dynamic dispatch occurs in hot loops.

No unimplemented GPU capability is advertised.

## Consequences

- Stable consumers can use reusable toolkit components.
- Full Stab performance retains portable SIMD.
- A later device backend must prove a real plan and batch contract before becoming public.
