# ADR 0003: Typed Batch Families

## Status

Accepted for Stab 0.2 architecture work. Revised on 2026-11-14: bit-plane batches are no longer limited to 64 shots, and frame lane width is a session parameter.

## Context

One generic record container would conflate measurement, detector, observable, sampled-error, and prediction semantics.

Shot-major packed records and bit planes also have different layout contracts.

Before the revision, bit planes were one `u64` per result bit and so held at most 64 shots. Circuit sampling ran 256-lane frames, detection sampling ran 64-lane frames, and DEM sampling produced up to 1024-shot planes, so each producer re-chunked its own planes into 64-shot batches before a sink saw them, and every row-oriented output transposed again inside the encoder.

## Decision

Use focused owned and borrowed batch families over shared checked packed storage.

Widths and layouts are explicit typed values.

Detection batches keep detector and observable planes separate.

DETS retains independent `M`, `D`, and `L` namespaces.

Codecs implement sinks over compatible batch views.

Record input streams decode into packed shot-major batches; the record-at-a-time visitors were removed because no product caller used them.

### Revision: One Multi-Word Plane View

`BitPlaneBatchView` is the one borrowed bit-plane layout. A plane holds one result bit for any number of shots, packed 64 shots per word, and the view borrows either planes stored back to back in words or one 256-lane `BitBlock` per plane, so engine frames are handed to sinks without copying. A word-aligned shot window selects part of a wider plane set. `BitPlaneBatch` is the owned form.

`RecordBatchView` names records of one width in either layout. Measurement, detection, and DEM-sample batch views carry `RecordBatchView` parts, so one detection batch may be bit planes from a sampler or shot-major records from a decoded file, and consumers read either through `get` or ask for the layout they need.

`RecordEncoder::write_batch` accepts parts in either layout, mixed freely within one record. It encodes bit planes 64 shots at a time: PTB64 copies whole plane words, and every other format transposes one 64-shot block per part into reused scratch. For a materializing session that transpose is the only layout conversion between a frame and the output bytes; a streaming run transposes in the engine instead, as the streaming revision below records.

Circuit sampling, detection sampling, measurement-to-detection conversion, and sweep correction share one frame batch state and one delivery loop. Every session delivers batches of at most `stab_engine::MAX_BATCH_SHOTS` (256) shots, and measurement-to-detection conversion accepts batches up to the same bound.

### Lane-Width Revision

A circuit-sampling or detection-sampling session fixes its frame lane width, a typed `stab_engine::LaneWidth`, when it is created: 256 lanes, one `BitBlock` per plane, by default, or 64 lanes, one `u64` per plane. Both widths run the same generic frame, Clifford kernels, noise sampling, conversion plan, and delivery loop, monomorphized per lane word, and the session dispatches on its width once per run. A 64-lane session delivers batches of at most 64 shots through the same `BitPlaneBatchView`, borrowing one plane word per result bit, so no new batch type exists. Measurement-to-detection conversion and sweep correction stay at 256 lanes because their batch size is the caller's.

The narrow width exists because a request for a few shots on a wide circuit otherwise samples and stores a full 256-lane batch: at distance 35 with 1000 rounds, `detect --shots 64` did four times the frame work it needed and held four times the plane storage. The lane width is a session property rather than a per-call choice, so partitioning a seeded run across calls on one session still reproduces the combined call; the seeded stream is a function of the plan, the seed, and the lane width. `LaneWidth::for_total_shots` picks the width for a known total: the width that samples the fewest lanes for the whole request, preferring the wider on a tie, for totals below 512 shots, and 256 lanes from there on. The CLI uses it for `sample` and `detect`. Generated DEM samples deliver 256-shot windows of their sample planes; DEM replay keeps its 64-record shot-major batches because it decodes one record at a time.

### Streaming Revision

A materializing session keeps one plane per measurement (and, for detection, per detector) for every lane of a batch, so its storage is the record width times the lane width even when a run delivers one shot; on a 5,000,000-round repetition code that was about 250 MB for one shot, where Stim streams in about 6 MB. `SamplingPlan::streaming_run` and `DetectionSamplingPlan::streaming_run` prepare one run of a known shot count that instead hands the frame's planes over in windows of about 1 MiB, between instructions. Each window is transposed in 64-plane tiles into shot-major records of only the lanes the run delivers, and detection resumes its conversion program over each window, so a run retains the frame, one window plus the deepest record lookback that a classical control, detector, or observable reads, and `min(shots, lane_width)` packed records. The sink receives shot-major batches, which every format except PTB64 encodes without further scratch. The run's output equals the first run of the same shots on a materializing session with the same seed and lane width, because every batch still samples all lanes; it drops the lanes it does not deliver, so it runs once. The CLI uses it for `sample` and `detect` requests of 1 to 63 shots, less than one batch of the narrowest lane width, where the saving is most of the lane width; a larger request would keep about as many record bits as planes, measured 5 to 15 percent slower when streamed, and so materializes, which also keeps every PTB64 request on bit planes.

## Consequences

- Engines and decoders can compose without text or nested boolean vectors.
- Layout conversions are explicit and benchmarkable.
- A future device backend may introduce another batch layout without weakening existing types.
- Sinks that need shot-major rows, such as decoders, read bit-plane batches from materializing sessions through `RecordBatchView::get` or transpose them themselves; only a streaming run transposes on their behalf, and it delivers shot-major batches.
- Sinks that preallocate per-batch storage must size it for `stab_engine::MAX_BATCH_SHOTS`, not 64.
