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
| 2 | executable-contract schema | big-endian `u16`, current value `7` |
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
| Direct Z measurement, historical schemas 1 through 5 | `1` |
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

Executable-contract schema version `5` replaces the redundant per-shot small-frame executor with a reusable 64-shot bit-plane Pauli frame for general compatible Clifford circuits. Unconsumed lanes remain in the session so partitioning a seeded run does not change its output stream. The existing scalar operation program remains the reference-sample and analysis representation, while unsupported circuits retain the general stabilizer-frame fallback.

Executable-contract schema version `6` compiles one resolved circuit program per plan and drives every executor from it: the bit-plane Pauli frame, the inverse-tableau reference sample, deterministic-measurement counting, and, until the limit policy reset, the general stabilizer frame. The separate per-target scalar operation program is retired. The direct Z measurement variant is removed, so a circuit whose only operation is one Z measurement, optionally preceded by a single-qubit Pauli channel on the measured qubit, now selects the bit-plane Pauli frame; its seeded output stream changes while its distribution does not. Until that reset the general stabilizer frame remained selected for a measurement-bearing program whose retained size exceeded the bit-plane frame's 256 MiB compiled budget. The reset removes that fallback: such a program is now refused with a typed compiled-bytes error against the caller's memory budget, so every admitted plan has the bit-plane Pauli-frame executable this schema already identifies, and the schema version is unchanged.

Current executable-contract schema version `7` keeps the schema-6 executable and makes the bit-plane Pauli frame's lane width a session parameter, the typed `LaneWidth` of 64 or 256 lanes. A plan's seeded stream is therefore a function of the plan, the seed, and the session's lane width; at the default 256 lanes it is byte-identical to schema 6, and partitioning a seeded run across calls on one session still reproduces the combined call at either width. The lane width is chosen per session, not per plan, so it is not part of the executable-contract digest; the CLI chooses it from the total shot count.

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
3eed620c6e6811fe2808572d8cd39917731ceaf3a99a4c50cc60507ed77716af
```

Scalar compilation selects the bit-plane Pauli frame variant.

The executable-contract digest is:

```text
74126cd438e4ea9b25331e6c2094e4a3b46d23acb11c4c59fbc31c249985f488
```

The final plan fingerprint is:

```text
8de0679e515d503b7a0db67cd6d8ef2f87a19571e97d2b46ac554fc4bcffa1a2
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
