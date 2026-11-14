# ADR 0002: Plans, Sessions, And Sinks

## Status

Accepted for Stab 0.2 architecture work.

## Context

Current compiled sampler types already lower source models, but they allocate mutable execution state inside convenience calls and know about serialized output formats.

That shape limits scratch reuse and direct composition with detection conversion or decoders.

## Decision

Each expensive execution family uses:

```text
compiler -> immutable plan -> mutable session -> typed sink
```

Plans are immutable, shareable, nonserializable, and carry summaries and versioned fingerprints.

Sessions own RNG state, frames, scratch, buffers, and progress counters.

Delivered batches contain at most `stab_engine::MAX_BATCH_SHOTS` (256) shots, one Pauli-frame batch at the widest lane width; DEM replay batches contain at most 64 records.

Circuit-sampling and detection-sampling sessions fix their Pauli-frame lane width, a typed `stab_engine::LaneWidth` of 64 or 256 lanes, when they are created, and deliver batches of at most that many shots. The default is 256 lanes. The seeded stream of a session is a function of its plan, seed, and lane width.

Successful chunking on one seeded session reproduces one combined call at every lane width.

Cancellation is checked between bounded internal batches.

Request rejection before work leaves a session reusable.

Sink write or finalization failure poisons the session because the accepted output prefix is unknown.

## Consequences

- Repeated execution reuses allocation.
- Execution no longer depends on codecs or files.
- Callers can compose simulation, conversion, decoding, and statistics.
- Cross-version, cross-backend, random-access, and exact Stim random streams remain outside the contract.
