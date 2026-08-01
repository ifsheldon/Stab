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

`stab-analysis` owns a generic `CircuitPass` contract only after the built-in without-noise transform and the separate `stab-reference-noise-pass` crate establish the common shape. Each pass owns typed options, a typed report, a typed diagnostic, and a checked conservative folded-output projection that cannot allocate in proportion to the projected output. The common executor alone can construct the admitted input value, applies caller-selected folded-model limits before dispatch, admits the output projection before lowering, exposes typed input/projection/output rejection stages, and validates the returned circuit against both the same closed Stim-compatible structural envelope and the projection before exposing it to the caller. Projected payload bytes exclude allocator metadata and spare capacity and therefore make no exact resident-memory claim.

The pass boundary accepts and returns `Circuit`. It does not add extension instructions, mutate the gate table, expose executable IR, or imply a runtime registry. Pass-specific inability to lower remains a typed pass diagnostic, while an attempted unknown gate is independently rejected by construction at the closed model boundary.

The `stab-core::experimental` facade tier reexports only the externally proven pass contract and built-in pass. The canonical Stable owner remains `stab-analysis`.

Sampling backend discovery and selection continue to use engine-owned descriptors and `BackendPreference`. `plan sample --backend` is an agent-facing adapter over that resolver. `auto` and explicit `scalar` currently resolve to the same scalar plan, while explicit `portable-simd` fails as unavailable until a genuinely distinct executable backend exists.

The future decoder process boundary is specified as requirements only in [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md). Stab 0.2 does not implement the transport or promise wire compatibility.

## Consequences

- Independent decoder crates can use static Cargo composition.
- The common session contract is based on real implementations.
- Python and external processes can later use a versioned protocol without freezing a Rust ABI.
- Independent circuit-pass crates can compose statically through public Stable model and analysis APIs.
- Pass output remains ordinary Stim-compatible circuit data, so downstream analysis and execution require no extension-aware branch.
- Backend requests cannot advertise or select an implementation that is absent from the executable registry.
