# Compilation Request Fingerprint Schema Version 1

This document defines the byte contract for Stab's backend-neutral `CompilationRequestFingerprint` schema version 1.

It complements the [model fingerprint schema](model-fingerprint-schema-v1.md).

## Purpose

A compilation request fingerprint identifies the semantic input to one compiler independently of the executable implementation recorded by the resulting plan.

It binds:

- the request-fingerprint schema;
- the compilation operation;
- the operation compiler schema;
- the complete model-fingerprint identity;
- normalized caller-selectable compilation options;
- effective caller-configurable compilation limits.

It does not bind execution inputs such as shot count, random seed, reference-sample mode, result encoding, or filesystem routing.

It also does not bind a backend preference, selected backend, or executable-plan identity. Backend selection happens after this identity is calculated, and the resulting [plan fingerprint](plan-fingerprint-schema-v1.md) binds the selected backend.

## Primitive Encoding

- Unsigned integers use fixed-width big-endian encoding.
- Sequence lengths use unsigned 128-bit big-endian encoding.
- SHA-256 digests appear as their 32 raw bytes, not hexadecimal text.
- There is no padding, alignment, separator byte, or terminal marker.

## Top-Level Encoding

The SHA-256 input is the following concatenation:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | domain | exact bytes `stab:compilation-request-fingerprint\0` |
| 2 | request-fingerprint schema | big-endian `u16`, value `1` |
| 3 | operation | one discriminator byte |
| 4 | compiler schema | big-endian `u16` |
| 5 | model-fingerprint schema | big-endian `u16` |
| 6 | model dialect | one model-dialect discriminator byte |
| 7 | model digest | 32 raw SHA-256 bytes |
| 8 | normalized option count | big-endian `u128` |
| 9 | normalized options | operation-specific entries in ascending field-id order |
| 10 | effective configurable-limit count | big-endian `u128` |
| 11 | effective configurable limits | operation-specific entries in ascending field-id order |

The model dialect discriminators are shared with model fingerprint schema version 1:

| Model dialect | Discriminator |
| --- | --- |
| Stim circuit | `1` |
| Detector error model | `2` |

## Sampling Compiler Schemas

Sampling uses operation discriminator `1`.

Historical compiler schema version `1` used one fixed lowering mode:

- sweep-controlled instructions are rejected;
- representability and semantic validation remain mandatory;
- no compile resource budget is caller-configurable;
- backend preference and selection are deliberately excluded because they do not change the backend-neutral lowering request.

Fixed compiler behavior belongs to the compiler schema, not to the normalized caller-option list.

Compiler schema version `2` changed only sweep admission:

- every legal sweep-controlled Pauli operation lowers into the same typed execution representation used by reference sampling and detection;
- sampling calls have no sweep-input parameter, so every omitted sweep bit is false;
- representability and semantic validation remain mandatory;
- no compile resource budget is caller-configurable;
- backend preference and selection remain excluded.

Compiler schema version `3` kept that sweep admission and additionally:

- represents record and sweep controls with one private typed operation;
- recognizes classical-bit/classical-bit `CZ` groups as unconditional no-ops before validating record history, matching Stim;
- permits the small-frame executable to retain omitted all-false sweep controls instead of selecting the general frame solely because a sweep target exists.

Compiler schema version `4` kept those semantics and additionally:

- retains compact repeat blocks in one validated flat operation tape instead of a recursively owned operation tree;
- compiles nested repeats with a fixed-depth iterative traversal so accepted nesting does not depend on the host thread stack;
- leaves the reference-repeat execution policy out of the semantic request identity because `fold` and `iterate` accept and lower the same circuit.

Current compiler schema version `5` retains schema `4` lowering and additionally admits at most one million expanded operation dispatches for a measurement-bearing shot. Zero-width circuits bypass represented execution work because no result can observe it. This is one aggregate execution-work boundary, not a repeat-count syntax cap.

All sampling compiler schemas encode:

- normalized option count `0`;
- effective configurable-limit count `0`.

Changing classical-control admission or semantic lowering again, adding a caller-selectable semantic lowering option, or adding a configurable compile limit requires a new sampling compiler schema and a corresponding request-fingerprint schema review. An execution-strategy option that preserves the lowered semantic program belongs to the plan's executable identity instead.

Adding or changing a backend registration does not by itself change this request schema. It changes the separate backend-bearing plan identity.

## Frozen Vector

The frozen circuit is:

```stim
X_ERROR[π](0.12345641) 0
M !1
CX sweep[7] 2
MPP !X0*Y1*!Z2
DETECTOR[coord](-0, 1.25) rec[-0] rec[-1]
REPEAT[loop] 3 {
    H 3
}
```

Its model fingerprint is:

```text
78361913886a45606681a49071b1689ad37758308655e69f28ed68675046f3dd
```

Its sampling compilation request fingerprint is:

```text
baeb18b2253a82defa506cf7cbd43d2ea6e1d5c51cbb0c4b8de88adca16cdde5
```

The request digest was independently reconstructed from the table above with Perl `Digest::SHA` and binary `pack`, rather than copied from the Rust implementation.

## Resource Behavior

Fingerprint generation streams a constant-size request header and an existing fixed-size model fingerprint into SHA-256.

It allocates no heap storage before hexadecimal rendering and does not compile or execute the model.

## Evolution Rule

Any change to the domain, field order, width, endianness, discriminator, option normalization, limit normalization, or included identity requires a schema-version change.

Changing compiler behavior that can change the lowered operation stream requires a compiler-schema change even when the request-fingerprint byte grammar itself remains unchanged.
