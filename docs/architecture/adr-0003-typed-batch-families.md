# ADR 0003: Typed Batch Families

## Status

Accepted for Stab 0.2 architecture work.

## Context

One generic record container would conflate measurement, detector, observable, sampled-error, and prediction semantics.

Shot-major packed records and 64-shot bit planes also have different layout contracts.

## Decision

Use focused owned and borrowed batch families over shared checked packed storage.

Widths and layouts are explicit typed values.

Detection batches keep detector and observable planes separate.

DETS retains independent `M`, `D`, and `L` namespaces.

Codecs implement sinks over compatible batch views.

Record-at-a-time visitors remain bounded adapters.

## Consequences

- Engines and decoders can compose without text or nested boolean vectors.
- Layout conversions are explicit and benchmarkable.
- A future device backend may introduce another batch layout without weakening existing types.
