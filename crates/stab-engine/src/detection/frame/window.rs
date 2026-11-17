//! Bounded windows of result planes for streaming frame batches.
//!
//! A streaming batch does not keep one plane per measurement (or detector) for the whole circuit.
//! The frame hands its recorded planes to a [`PlaneConsumer`] whenever a fixed-size window has
//! filled, between two instructions, and then drops every plane that neither the consumer nor the
//! program's own classical controls can still read. [`ShotRecordTransposer`] is the consumer that
//! turns those windows into shot-major records of only the lanes a run delivers.

use stab_bits::transpose_bit_tile;
use stab_records::{PackedShotBatch, PackedShotBatchView};

use super::program::CircuitProgram;
use super::word::FrameWord;
use crate::detection::error::{DetectionError, DetectionResult};
use crate::work_poll::{WORD_OP_UNITS, WorkPoll};

/// Bytes of planes one window holds before it is handed over, independent of the circuit.
pub(crate) const WINDOW_TARGET_BYTES: usize = 1 << 20;

const WORD_BITS: usize = u64::BITS as usize;

/// Planes transposed per scratch group: 64 tiles of 64 planes, 32 KiB of scratch.
const GROUP_TILES: usize = 64;
const GROUP_PLANES: usize = GROUP_TILES * WORD_BITS;

/// Scratch bytes of one [`ShotRecordTransposer`].
pub(crate) const TRANSPOSE_SCRATCH_BYTES: usize = GROUP_PLANES * size_of::<u64>();

/// Planes in one window of `plane_bytes`-byte planes: a positive multiple of 64.
pub(crate) fn window_planes(plane_bytes: usize) -> usize {
    #[cfg(test)]
    if let Some(planes) = WINDOW_PLANES_OVERRIDE.get() {
        return planes;
    }
    let planes = WINDOW_TARGET_BYTES / plane_bytes.max(1);
    let aligned = planes - planes % WORD_BITS;
    if aligned == 0 { WORD_BITS } else { aligned }
}

#[cfg(test)]
thread_local! {
    static WINDOW_PLANES_OVERRIDE: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

/// Runs `body` with windows of `planes` planes (a positive multiple of 64) on this thread, so
/// tests cross many window boundaries on small circuits.
#[cfg(test)]
pub(crate) fn with_window_planes<T>(planes: usize, body: impl FnOnce() -> T) -> T {
    assert!(
        planes > 0 && planes.is_multiple_of(WORD_BITS),
        "window planes must be whole tiles"
    );
    WINDOW_PLANES_OVERRIDE.set(Some(planes));
    let result = body();
    WINDOW_PLANES_OVERRIDE.set(None);
    result
}

/// How one streaming frame bounds the result planes it retains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowShape {
    /// Planes recorded between two hand-overs.
    window_planes: usize,
    /// Newest planes the frame keeps after a hand-over for its own classical controls.
    feedback_lookback: usize,
    /// Most planes the frame retains at once, never more than the record's width.
    capacity: usize,
}

impl WindowShape {
    /// The shape for `program`'s measurement planes of `plane_bytes` bytes, when the consumer
    /// may still read up to `consumer_lookback` planes behind the newest one at a hand-over.
    pub(crate) fn for_program(
        program: &CircuitProgram,
        plane_bytes: usize,
        consumer_lookback: usize,
    ) -> Self {
        let needs = program.measurement_window_needs();
        let window_planes = window_planes(plane_bytes);
        // After a hand-over at most `lookback + 63` planes stay (63 for a consumer that
        // releases whole tiles); the next hand-over happens once `window_planes` more were
        // recorded, at the end of an instruction that can add `largest_instruction` at once.
        let capacity = needs
            .feedback_lookback
            .max(consumer_lookback)
            .saturating_add(WORD_BITS - 1)
            .saturating_add(window_planes)
            .saturating_add(needs.largest_instruction)
            .min(program.counts().measurements);
        Self {
            window_planes,
            feedback_lookback: needs.feedback_lookback,
            capacity,
        }
    }

    pub(crate) const fn capacity(self) -> usize {
        self.capacity
    }
}

/// Receives the retained planes of a streaming frame at every hand-over.
pub(crate) trait PlaneConsumer<W> {
    /// `planes` holds the planes with absolute indices `base..base + planes.len()`; `last` marks
    /// the final hand-over of the batch, after which every plane must have been consumed.
    ///
    /// Returns the absolute index before which the consumer will never read a plane again. The
    /// consumer charges `poll` for its work, so a hand-over of a wide window stays cancellable.
    fn consume(
        &mut self,
        base: usize,
        planes: &[W],
        last: bool,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<usize>;
}

/// A frame's view of its consumer while one streaming batch executes.
pub(crate) struct PlaneWindow<'a, W> {
    pub(super) shape: WindowShape,
    pub(super) consumer: &'a mut dyn PlaneConsumer<W>,
    /// Retained plane count at which the next hand-over happens.
    pub(super) hand_over_at: usize,
}

impl<'a, W> PlaneWindow<'a, W> {
    pub(crate) fn new(shape: WindowShape, consumer: &'a mut dyn PlaneConsumer<W>) -> Self {
        Self {
            shape,
            consumer,
            hand_over_at: shape.window_planes,
        }
    }

    /// Hands `planes` over and drops the ones nobody can read again; returns the new base.
    pub(super) fn hand_over(
        &mut self,
        base: usize,
        planes: &mut Vec<W>,
        last: bool,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<usize> {
        let released = self.consumer.consume(base, planes, last, poll)?;
        if last {
            return Ok(base);
        }
        let end = base + planes.len();
        let keep_from = released
            .min(end.saturating_sub(self.shape.feedback_lookback))
            .max(base);
        planes.drain(..keep_from - base);
        self.hand_over_at = planes.len().saturating_add(self.shape.window_planes);
        Ok(keep_from)
    }
}

/// Shot-major records of the first lanes of a frame batch, transposed window by window.
///
/// Rows are reused across batches: every word below a batch's record width is overwritten, and
/// the words of one row past the width stay zero.
#[derive(Debug)]
pub(crate) struct ShotRecordTransposer {
    records: PackedShotBatch,
    lanes: usize,
    /// Absolute plane index below which this batch's planes are transposed.
    done: usize,
    scratch: Vec<[u64; WORD_BITS]>,
}

impl ShotRecordTransposer {
    /// Storage for batches of at most `max_lanes` delivered lanes of `width`-bit records.
    pub(crate) fn try_new(max_lanes: usize, width: usize) -> DetectionResult<Self> {
        let records = PackedShotBatch::zeros(max_lanes, width).map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "streaming records of {max_lanes} shots and {width} bits: {error}"
            ))
        })?;
        let mut scratch = Vec::new();
        scratch.try_reserve_exact(GROUP_TILES).map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "streaming transpose scratch: {error}"
            ))
        })?;
        Ok(Self {
            records,
            lanes: 0,
            done: 0,
            scratch,
        })
    }

    /// Bytes [`Self::try_new`] allocates.
    pub(crate) fn storage_bytes(max_lanes: usize, width: usize) -> u128 {
        (max_lanes as u128)
            .saturating_mul(width.div_ceil(WORD_BITS) as u128)
            .saturating_mul(size_of::<u64>() as u128)
            .saturating_add(TRANSPOSE_SCRATCH_BYTES as u128)
    }

    /// Grows the records to hold at least `lanes` shots, releasing the old ones first; the
    /// caller admitted the largest batch it converts when it created this transposer.
    pub(crate) fn ensure_lanes(&mut self, lanes: usize) -> DetectionResult<()> {
        if self.records.shot_count() >= lanes {
            return Ok(());
        }
        let width = self.records.bits_per_shot();
        self.records = PackedShotBatch::zeros(0, width).map_err(records_error)?;
        self.records = PackedShotBatch::zeros(lanes, width).map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "streaming records of {lanes} shots and {width} bits: {error}"
            ))
        })?;
        Ok(())
    }

    /// Starts a batch that delivers its first `lanes` lanes.
    pub(crate) fn begin_batch(&mut self, lanes: usize) -> DetectionResult<()> {
        if lanes > self.records.shot_count() {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "a streaming batch delivers {lanes} lanes but its records hold {}",
                self.records.shot_count()
            )));
        }
        self.lanes = lanes;
        self.done = 0;
        Ok(())
    }

    /// The finished records of the current batch.
    pub(crate) fn view(&self) -> DetectionResult<PackedShotBatchView<'_>> {
        if self.done != self.records.bits_per_shot() {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "a streaming batch transposed {} of {} result planes",
                self.done,
                self.records.bits_per_shot()
            )));
        }
        self.records
            .view_prefix(self.lanes)
            .map_err(|error| DetectionError::invalid_sampler_compilation(error.to_string()))
    }

    /// Transposes the planes from the last transposed one up to `upto` (a multiple of 64 unless
    /// it is the record's end), XOR-ing `flips` (packed, one bit per plane) into every record.
    /// Each scratch group of planes is charged to `poll` before it is transposed.
    pub(crate) fn transpose_until<W: FrameWord>(
        &mut self,
        base: usize,
        planes: &[W],
        upto: usize,
        flips: &[u64],
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        if self.done < base || upto < self.done || upto > base + planes.len() {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "streaming transpose of planes {}..{upto} escaped its window {base}..{}",
                self.done,
                base + planes.len()
            )));
        }
        let mut start = self.done;
        let blocks = self.lanes.div_ceil(WORD_BITS);
        while start < upto {
            let group_end = upto.min(start + GROUP_PLANES);
            poll.charge((group_end - start) * blocks * WORD_OP_UNITS)?;
            self.transpose_group(base, planes, start, group_end, flips)?;
            start = group_end;
        }
        self.done = upto;
        Ok(())
    }

    fn transpose_group<W: FrameWord>(
        &mut self,
        base: usize,
        planes: &[W],
        start: usize,
        end: usize,
        flips: &[u64],
    ) -> DetectionResult<()> {
        let tiles = (end - start).div_ceil(WORD_BITS);
        let first_word = start / WORD_BITS;
        let group = start
            .checked_sub(base)
            .and_then(|from| planes.get(from..end - base))
            .ok_or_else(window_escape)?;
        // The group's flip words, zero past the end of `flips`, so the row loop below reads
        // three equally long sequences and compiles without per-word bounds checks.
        let mut group_flips = [0_u64; GROUP_TILES];
        for (slot, flip) in group_flips
            .iter_mut()
            .zip(flips.get(first_word..).unwrap_or_default())
        {
            *slot = *flip;
        }
        self.scratch.resize(tiles, [0; WORD_BITS]);
        for block in 0..self.lanes.div_ceil(WORD_BITS) {
            for (tile, source) in self.scratch.iter_mut().zip(group.chunks(WORD_BITS)) {
                let mut source = source.iter();
                for slot in tile.iter_mut() {
                    *slot = source.next().map_or(0, |plane| plane.lane_word(block));
                }
                transpose_bit_tile(tile);
            }
            let block_lanes = (self.lanes - block * WORD_BITS).min(WORD_BITS);
            for lane in 0..block_lanes {
                let mut row = self
                    .records
                    .shot_words_mut(block * WORD_BITS + lane)
                    .map_err(|error| {
                        DetectionError::invalid_sampler_compilation(error.to_string())
                    })?;
                let target = row
                    .get_mut(first_word..first_word + tiles)
                    .ok_or_else(window_escape)?;
                for ((word, tile), flip) in target.iter_mut().zip(&self.scratch).zip(&group_flips) {
                    *word = tile.get(lane).copied().unwrap_or(0) ^ *flip;
                }
            }
        }
        Ok(())
    }
}

/// The consumer of a sampling batch: transposes every plane, flipping the reference bits.
pub(crate) struct SampledRecords<'a> {
    pub(crate) records: &'a mut ShotRecordTransposer,
    pub(crate) reference: &'a [u64],
}

impl<W: FrameWord> PlaneConsumer<W> for SampledRecords<'_> {
    fn consume(
        &mut self,
        base: usize,
        planes: &[W],
        last: bool,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<usize> {
        let end = base + planes.len();
        let upto = if last { end } else { end - end % WORD_BITS };
        self.records
            .transpose_until(base, planes, upto, self.reference, poll)?;
        Ok(upto)
    }
}

fn records_error(error: stab_records::FormatError) -> DetectionError {
    DetectionError::invalid_sampler_compilation(error.to_string())
}

fn window_escape() -> DetectionError {
    DetectionError::invalid_sampler_compilation("a streaming transpose escaped its plane window")
}
