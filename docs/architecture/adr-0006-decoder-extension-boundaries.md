# ADR 0006: Decoder And External Extension Boundaries

## Status

Accepted for Stab 0.2 architecture work.

## Context

QEC workflows need to compose sampled detection events with independently developed decoders.

Rust dynamic-library ABIs are not stable, and designing a universal decoder compiler before a real integration would be speculative.

## Decision

`stab-decoder` owns stable detection-input, observable-prediction, and decoder-session interoperability.

Decoder compilation remains implementation-specific in 0.2.

A separate bounded exact repetition-code decoder proves the Rust seam using public Stable component crates only.

A process protocol is documented after the Rust contract is proven but is not implemented in 0.2.

Dynamic Rust plugins are forbidden.

## Consequences

- Independent decoder crates can use static Cargo composition.
- The common session contract is based on real implementations.
- Python and external processes can later use a versioned protocol without freezing a Rust ABI.
