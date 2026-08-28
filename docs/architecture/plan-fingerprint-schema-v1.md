# Plan Fingerprint Schema Version 1

This document defines the byte contract for Stab's backend-bearing `PlanFingerprint` schema version 1.

It complements the backend-neutral [compilation request fingerprint schema](compilation-request-fingerprint-schema-v1.md).

## Purpose

A plan fingerprint identifies the immutable executable selected for one compilation request.

It binds:

- the plan-fingerprint schema;
- the complete compilation-request fingerprint;
- the selected execution backend;
- the executable-contract schema and digest.

It does not bind mutable session state, random seed, shot count, reference-sample mode, result encoding, cancellation state, sink choice, filesystem routing, or serialized plan bytes.

Compiled plans are not serializable.

## Primitive Encoding

- Unsigned integers use fixed-width big-endian encoding.
- SHA-256 digests appear as their 32 raw bytes, not hexadecimal text.
- Discriminators occupy one byte.
- There is no padding, alignment, separator byte, or terminal marker.

## Executable Contract Digest

Schema version 1 first calculates an executable-contract SHA-256 digest from:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | domain | exact bytes `stab:sampling-executable-contract\0` |
| 2 | executable-contract schema | big-endian `u16`, current value `5` |
| 3 | backend | one discriminator byte |
| 4 | private executable variant | one discriminator byte |
| 5 | reference-sample loop policy | one discriminator byte |

Backend discriminators are:

| Backend | Discriminator | Schema-1 availability |
| --- | --- | --- |
| Scalar | `1` | Registered |
| Portable SIMD | `2` | Reserved; registration is deferred until a distinct measured engine plan exists |

Private executable-variant discriminators are:

| Variant | Discriminator |
| --- | --- |
| Direct Z measurement | `1` |
| Small stabilizer frame, historical schemas 1 through 4 | `2` |
| General stabilizer frame | `3` |
| Bit-plane Pauli frame | `4` |

The variant remains private even though its discriminator participates in identity.

Reference-sample loop-policy discriminators are:

| Policy | Discriminator |
| --- | --- |
| Fold invariant repeats | `1` |
| Iterate every repeat | `2` |

Changing executable selection or semantics without changing one of these bound identities is forbidden.

### Executable-Contract Schema History

Historical executable-contract schema version `1` used separate record-feedback and sweep-control operations. Omitted sweep controls selected the general frame because the small-frame executor did not own their all-false semantics.

Executable-contract schema version `2` uses one classically controlled Pauli operation for record and sweep controls, recognizes all-classical `CZ` pairs as unconditional no-ops, and permits the small-frame executor to retain omitted all-false sweeps.

Executable-contract schema version `3` retains repeats in one validated flat operation tape and binds the reference-sample loop policy. `Fold` may reuse a repeat's output pattern only after proving exact stabilizer-state recurrence and only when its optional snapshot fits the existing admitted session-storage ceiling; otherwise it executes every represented iteration. `Iterate` always executes every represented iteration. These policies preserve semantic output but authorize different executable work and therefore have different plan identities even when a particular `Fold` plan conservatively iterates.

Executable-contract schema version `4` gives zero-width sampling a constant-work execution path and removes frame allocation for that path. Measurement-bearing plans retain schema `3` execution after compiler-schema-5 aggregate work admission.

Current executable-contract schema version `5` replaces the redundant per-shot small-frame executor with a reusable 64-shot bit-plane Pauli frame for general compatible Clifford circuits. Unconsumed lanes remain in the session so partitioning a seeded run does not change its output stream. The existing scalar operation program remains the reference-sample and analysis representation, while unsupported circuits retain the general stabilizer-frame fallback.

## Plan Fingerprint Encoding

The final SHA-256 input is the following concatenation:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | domain | exact bytes `stab:plan-fingerprint\0` |
| 2 | plan-fingerprint schema | big-endian `u16`, value `1` |
| 3 | request-fingerprint schema | big-endian `u16` |
| 4 | request fingerprint | 32 raw SHA-256 bytes |
| 5 | selected backend | one discriminator byte |
| 6 | executable-contract schema | big-endian `u16` |
| 7 | executable-contract digest | 32 raw SHA-256 bytes |

The request fingerprint already binds the compiler operation, compiler schema, model identity, normalized lowering options, and effective configurable compilation limits.

## Frozen Vector

The frozen circuit is:

```stim
M 0
```

Its sampling compilation request fingerprint is:

```text
985c1f3cfc8642113bb68568a71508ec46f0d39fa7d918fed9b75fa3764d4b79
```

Scalar compilation selects the direct Z measurement variant.

The executable-contract digest is:

```text
c80923552aa640482af211b7f6de03580b9311f42685c971f7b0f53355d4bbdf
```

The final plan fingerprint is:

```text
12d6f2077cf0e77f15690476071bb5c1a2a56e1039956c4c62a71b71961911d0
```

The test reconstructs both digests independently from the tables above instead of calling the production fingerprint constructor.

## Comparability

Plan fingerprints are directly comparable only when the plan-fingerprint schema and backend identity match.

An identical digest implies an identical request and executable identity under that schema. It does not imply that two sessions have the same random state or output position.

## Resource Behavior

Plan fingerprint generation hashes fixed-size identities after lowering selects an executable variant.

It allocates no model-sized buffer and does not execute a shot.

Hexadecimal rendering allocates the returned string.

## Evolution Rule

Any change to a domain, field order, width, endianness, discriminator, executable-selection meaning, or included identity requires the corresponding schema-version change.

A backend implementation change that can alter executable semantics requires an executable-contract schema change even when the public plan byte grammar remains unchanged.
