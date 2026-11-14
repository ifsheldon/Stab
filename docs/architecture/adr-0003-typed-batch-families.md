# ADR 0003: Typed Batch Families

## Status

Accepted for Stab 0.2 architecture work. Revised by E7 of the [engine speed and architecture plan](../plans/engine-speed-and-architecture-plan.md): bit-plane batches are no longer limited to 64 shots.

## Context

One generic record container would conflate measurement, detector, observable, sampled-error, and prediction semantics.

Shot-major packed records and bit planes also have different layout contracts.

Before E7, bit planes were one `u64` per result bit and so held at most 64 shots. Circuit sampling ran 256-lane frames, detection sampling ran 64-lane frames, and DEM sampling produced up to 1024-shot planes, so each producer re-chunked its own planes into 64-shot batches before a sink saw them, and every row-oriented output transposed again inside the encoder.

## Decision

Use focused owned and borrowed batch families over shared checked packed storage.

Widths and layouts are explicit typed values.

Detection batches keep detector and observable planes separate.

DETS retains independent `M`, `D`, and `L` namespaces.

Codecs implement sinks over compatible batch views.

Record input streams decode into packed shot-major batches; the record-at-a-time visitors were removed in E6 because no product caller used them.

### E7 Revision: One Multi-Word Plane View

`BitPlaneBatchView` is the one borrowed bit-plane layout. A plane holds one result bit for any number of shots, packed 64 shots per word, and the view borrows either planes stored back to back in words or one 256-lane `BitBlock` per plane, so engine frames are handed to sinks without copying. A word-aligned shot window selects part of a wider plane set. `BitPlaneBatch` is the owned form.

`RecordBatchView` names records of one width in either layout. Measurement, detection, and DEM-sample batch views carry `RecordBatchView` parts, so one detection batch may be bit planes from a sampler or shot-major records from a decoded file, and consumers read either through `get` or ask for the layout they need.

`RecordEncoder::write_batch` accepts parts in either layout, mixed freely within one record. It encodes bit planes 64 shots at a time: PTB64 copies whole plane words, and every other format transposes one 64-shot block per part into reused scratch. That transpose is the only layout conversion between a frame and the output bytes.

Circuit sampling, detection sampling, measurement-to-detection conversion, and sweep correction share one 256-lane frame batch and one delivery loop. Every session delivers batches of at most `stab_engine::MAX_BATCH_SHOTS` (256) shots, and measurement-to-detection conversion accepts batches up to the same bound. Generated DEM samples deliver 256-shot windows of their sample planes; DEM replay keeps its 64-record shot-major batches because it decodes one record at a time.

## Consequences

- Engines and decoders can compose without text or nested boolean vectors.
- Layout conversions are explicit and benchmarkable.
- A future device backend may introduce another batch layout without weakening existing types.
- Sinks that need shot-major rows, such as decoders, read bit-plane batches through `RecordBatchView::get` or transpose them themselves; the engine no longer transposes on their behalf.
- Sinks that preallocate per-batch storage must size it for `stab_engine::MAX_BATCH_SHOTS`, not 64.
