# Model Fingerprint Schema Version 1

This document is the normative byte-level contract for `ModelFingerprint::SCHEMA_VERSION == 1`.

Its purpose is independent reproduction. The `.stim` and `.dem` compatibility printers are not part of this identity because their intentional floating-point rounding can merge distinct model values.

## Hash Envelope

The digest is SHA-256 over this exact concatenation:

```text
ASCII("stab:model-fingerprint") || u8(0) || u16(1) || dialect || model
```

All integers are big-endian. The dialect is one byte:

| Value | Dialect |
| --- | --- |
| `1` | Stim circuit |
| `2` | Detector error model |

`ModelFingerprint` equality includes the schema version, dialect, and 32 digest bytes.

## Primitive Encodings

| Value | Encoding |
| --- | --- |
| Boolean false or true | `u8(0)` or `u8(1)` |
| Signed integer | Fixed-width two's-complement big-endian integer |
| Unsigned integer | Fixed-width big-endian integer |
| Length | `u128` containing an item count, except that a text length counts UTF-8 bytes |
| Text | UTF-8 byte length followed by those exact bytes; no Unicode normalization |
| Optional text | `u8(0)` for absent, or `u8(1)` followed by text |
| Float sequence | `u128` item count followed by one `u64` per `f64` value |

Each nonzero `f64` is encoded with `f64::to_bits()`. Both signed zeros encode as `u64(0)`. No other floating-point normalization or printer rounding occurs.

Every sequence begins with its `u128` item count. Empty sequences therefore encode as sixteen zero bytes.

## Stim Circuit Model

A circuit is the item count followed by its items in model order.

| Circuit item | Discriminator and fields in exact order |
| --- | --- |
| Instruction | `u8(1)`, canonical gate-name text, float sequence, target count and targets, optional tag |
| Repeat block | `u8(2)`, repeat count as `u64`, optional tag, nested circuit |

Gate aliases and accepted case variants resolve to the gate's canonical name before fingerprinting. Repeat blocks remain folded.

Circuit targets use these encodings:

| Target | Discriminator and fields in exact order |
| --- | --- |
| Qubit | `u8(1)`, qubit ID as `u32`, inversion Boolean |
| Measurement record | `u8(2)`, offset as `i32`, parsed-negative-zero Boolean |
| Sweep bit | `u8(3)`, sweep-bit ID as `u32` |
| Pauli target | `u8(4)`, Pauli discriminator, qubit ID as `u32`, inversion Boolean |
| Combiner | `u8(5)` |

Pauli discriminators are `1` for X, `2` for Y, and `3` for Z. The parsed-negative-zero Boolean distinguishes Stim's retained `rec[-0]` representation from ordinary signed offsets.

## Detector Error Model

A detector error model is the item count followed by its items in model order.

| DEM item | Discriminator and fields in exact order |
| --- | --- |
| Instruction | `u8(1)`, instruction-kind discriminator, float sequence, target count and targets, optional tag |
| Repeat block | `u8(2)`, repeat count as `u64`, optional tag, nested detector error model |

Instruction-kind discriminators are:

| Value | Kind |
| --- | --- |
| `1` | `error` |
| `2` | `detector` |
| `3` | `logical_observable` |
| `4` | `shift_detectors` |

DEM targets use these encodings:

| Target | Discriminator and fields in exact order |
| --- | --- |
| Relative detector | `u8(1)`, detector ID as `u64` |
| Logical observable | `u8(2)`, observable ID as `u64` |
| Separator | `u8(3)` |
| Numeric shift | `u8(4)`, value as `u64` |

## Frozen Vectors

This circuit:

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

has digest:

```text
78361913886a45606681a49071b1689ad37758308655e69f28ed68675046f3dd
```

This detector error model:

```text
error[ε](0.12345641) D0 L1 ^ D2
detector[coord](-0, 2.5) D3
logical_observable[obs] L4
shift_detectors[shift](-0, 7.25) 9
repeat[loop] 3 {
    error(0.25) D5
}
```

has digest:

```text
a9da2cfcc5bbb92bdf4f50a9da5a5669f7f50909baaf7700a9aece756d554c65
```

Both expected values were independently reconstructed with Perl `Digest::SHA` and binary `pack` operations instead of calling Stab's encoder.

## Resource Contract

The implementation streams this encoding directly into SHA-256 and never materializes model-sized canonical text or bytes. Traversal storage is linear in repeat depth, not model volume. The root plus all 256 parser-admitted repeat levels fit inline without heap allocation; models constructed programmatically beyond that envelope spill only additional traversal frames to the heap.

## Evolution Rule

Changing field order, widths, byte order, length units, discriminators, Boolean representation, float normalization, canonical gate naming, tag handling, or any existing model-variant encoding requires a schema-version bump. A new schema must publish its own complete contract and frozen vectors. Existing schema-one fingerprints must never silently change.
