//! Measurement-to-detection conversion of one batch through bounded windows of planes.
//!
//! A materializing conversion transposes a whole batch into one 256-lane plane per measurement
//! and per detector before it converts. Without sweep correction no plane outlives the
//! detectors and observables that read it, so a [`WindowedConversion`] transposes the batch's
//! measurement columns a window at a time, resumes the conversion program over each window,
//! and transposes every full detector window into shot-major records of the batch's shots. A
//! batch then costs one window of planes plus its own detector records, not one plane per
//! measurement and detector for 256 lanes.

use stab_bits::{BIT_BLOCK_WORDS, BitBlock, transpose_bit_tile};
use stab_model::Cancellation;
use stab_records::{
    BitPlaneBatchView, DetectionBatchView, MeasurementBatchView, PackedShotBatchView,
};

use super::conversion_plan::{ConversionPlan, ConversionRunner};
use super::error::{DetectionError, DetectionResult};
use super::frame::{
    FRAME_BATCH_SHOTS, FrameLanes, FrameWord, PlaneConsumer, ShotRecordTransposer, window_planes,
};
use super::streaming::DetectionWindow;
use super::try_vec_with_capacity;
use crate::ReferenceSample;
use crate::work_poll::{WORD_OP_UNITS, WorkPoll};

const WORD_BITS: usize = u64::BITS as usize;

/// Reusable storage of windowed conversion for one plan.
#[derive(Debug)]
pub(super) struct WindowedConversion {
    /// Retained measurement planes: the lookback of the last hand-over and one new window.
    planes: Vec<FrameLanes>,
    window_planes: usize,
    lookback: usize,
    detector_window: Vec<FrameLanes>,
    observable_planes: Vec<FrameLanes>,
    detectors: ShotRecordTransposer,
    observables: ShotRecordTransposer,
}

/// Plane counts of one plan's windowed conversion.
#[derive(Clone, Copy, Debug)]
struct WindowCounts {
    window_planes: usize,
    capacity: usize,
    detector_window: usize,
    lookback: usize,
}

impl WindowCounts {
    fn new(plan: &ConversionPlan) -> Self {
        let window_planes = window_planes(size_of::<FrameLanes>());
        let lookback = plan.max_record_lookback();
        Self {
            window_planes,
            capacity: lookback
                .saturating_add(window_planes)
                .min(plan.measurement_count),
            detector_window: window_planes.min(plan.detector_count),
            lookback,
        }
    }
}

impl WindowedConversion {
    /// Conservative storage of [`Self::try_new`] and of the largest batch it converts: the
    /// window planes, the detector window, the observable planes, and shot-major detector and
    /// observable records of [`FRAME_BATCH_SHOTS`] shots with their transpose scratch.
    pub(super) fn storage_bytes(plan: &ConversionPlan) -> u128 {
        let counts = WindowCounts::new(plan);
        let planes = (counts.capacity as u128)
            .saturating_add(counts.detector_window as u128)
            .saturating_add(plan.observable_count as u128);
        planes
            .saturating_mul(size_of::<FrameLanes>() as u128)
            .saturating_add(ShotRecordTransposer::storage_bytes(
                FRAME_BATCH_SHOTS,
                plan.detector_count,
            ))
            .saturating_add(ShotRecordTransposer::storage_bytes(
                FRAME_BATCH_SHOTS,
                plan.observable_count,
            ))
    }

    /// Reserves the windows; the records grow to each batch's shot count when it arrives.
    pub(super) fn try_new(plan: &ConversionPlan) -> DetectionResult<Self> {
        let counts = WindowCounts::new(plan);
        let planes = try_vec_with_capacity(counts.capacity, "windowed conversion planes")?;
        let mut detector_window =
            try_vec_with_capacity(counts.detector_window, "windowed conversion detectors")?;
        detector_window.resize(counts.detector_window, FrameLanes::ZERO);
        let mut observable_planes =
            try_vec_with_capacity(plan.observable_count, "windowed conversion observables")?;
        observable_planes.resize(plan.observable_count, FrameLanes::ZERO);
        Ok(Self {
            planes,
            window_planes: counts.window_planes,
            lookback: counts.lookback,
            detector_window,
            observable_planes,
            detectors: ShotRecordTransposer::without_lanes(plan.detector_count)?,
            observables: ShotRecordTransposer::without_lanes(plan.observable_count)?,
        })
    }

    /// Converts the `shot_count` records of `measurements` with `reference` flipped in, leaving
    /// their detector and observable records for [`Self::batch_view`].
    pub(super) fn convert(
        &mut self,
        plan: &ConversionPlan,
        measurements: MeasurementBatchView<'_>,
        shot_count: usize,
        reference: Option<&ReferenceSample>,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        let input = BatchColumns::new(measurements, shot_count)?;
        if let Some(reference) = reference
            && reference.len() != plan.measurement_count
        {
            return Err(DetectionError::invalid_result_format(format!(
                "reference sample has {} bits but the plan converts {} measurements",
                reference.len(),
                plan.measurement_count
            )));
        }
        let mut poll = WorkPoll::new(cancellation);
        self.detectors.ensure_lanes(shot_count, &mut poll)?;
        self.observables.ensure_lanes(shot_count, &mut poll)?;
        self.detectors.begin_batch(shot_count)?;
        self.observables.begin_batch(shot_count)?;
        self.observable_planes.fill(FrameLanes::ZERO);
        self.planes.clear();
        let active = FrameLanes::active_mask(shot_count);
        let mut consumer = DetectionWindow::new(
            plan,
            ConversionRunner::default(),
            self.lookback,
            &mut self.detector_window,
            &mut self.detectors,
            &mut self.observable_planes,
            cancellation,
        );
        let mut base = 0_usize;
        let mut next = 0_usize;
        loop {
            let end = plan.measurement_count.min(next + self.window_planes);
            input.append_planes(next, end, reference, active, &mut self.planes, &mut poll)?;
            next = end;
            let last = next == plan.measurement_count;
            let released = consumer.consume(base, &self.planes, last, &mut poll)?;
            if last {
                break;
            }
            let keep_from = released.clamp(base, base + self.planes.len());
            self.planes.drain(..keep_from - base);
            base = keep_from;
        }
        let observable_count = self.observable_planes.len();
        self.observables.transpose_until(
            0,
            &self.observable_planes,
            observable_count,
            &[],
            &mut poll,
        )
    }

    /// The detector and observable records of the last converted batch.
    pub(super) fn batch_view(&self) -> DetectionResult<DetectionBatchView<'_>> {
        DetectionBatchView::try_new(self.detectors.view()?, self.observables.view()?)
            .map_err(|error| DetectionError::invalid_sampler_compilation(error.to_string()))
    }
}

/// The columns of one input batch, in whichever layout it arrived.
enum BatchColumns<'a> {
    /// The records of each 64-shot block, one row slice per shot.
    Rows(Vec<[&'a [u64]; WORD_BITS]>),
    /// One plane per measurement.
    Planes(BitPlaneBatchView<'a>),
}

impl<'a> BatchColumns<'a> {
    fn new(batch: MeasurementBatchView<'a>, shot_count: usize) -> DetectionResult<Self> {
        if batch.shot_count() != shot_count || shot_count > FRAME_BATCH_SHOTS {
            return Err(DetectionError::invalid_result_format(format!(
                "windowed conversion of {shot_count} shots received a batch of {}",
                batch.shot_count()
            )));
        }
        if let Some(planes) = batch.bit_planes() {
            return Ok(Self::Planes(planes));
        }
        let records = batch.shot_major_records().ok_or_else(|| {
            DetectionError::invalid_result_format(
                "measurement batch has no recognized storage representation",
            )
        })?;
        Ok(Self::Rows(block_rows(records, shot_count)?))
    }

    /// Appends the planes of measurements `start..end` (`start` a multiple of 64) to `planes`,
    /// flipping the active lanes of each plane whose `flips` entry is set.
    fn append_planes(
        &self,
        start: usize,
        end: usize,
        reference: Option<&ReferenceSample>,
        active: FrameLanes,
        planes: &mut Vec<FrameLanes>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let tile_units = WORD_BITS * BIT_BLOCK_WORDS * WORD_OP_UNITS;
        let mut tile_start = start;
        let mut flips = reference.map(|reference| reference.iter().skip(start));
        while tile_start < end {
            let tile_end = end.min(tile_start + WORD_BITS);
            poll.charge(tile_units)?;
            let mut words = [[0_u64; WORD_BITS]; BIT_BLOCK_WORDS];
            match self {
                Self::Rows(blocks) => {
                    let word_index = tile_start / WORD_BITS;
                    for (tile, rows) in words.iter_mut().zip(blocks) {
                        for (slot, row) in tile.iter_mut().zip(rows) {
                            *slot = row.get(word_index).copied().unwrap_or(0);
                        }
                        transpose_bit_tile(tile);
                    }
                }
                Self::Planes(view) => {
                    for (offset, measurement) in (tile_start..tile_end).enumerate() {
                        let plane = view.plane(measurement).map_err(input_error)?;
                        for (tile, word) in words.iter_mut().zip(plane.words()) {
                            if let Some(slot) = tile.get_mut(offset) {
                                *slot = *word;
                            }
                        }
                    }
                }
            }
            for offset in 0..tile_end - tile_start {
                let lanes = std::array::from_fn(|block| {
                    words
                        .get(block)
                        .and_then(|tile| tile.get(offset))
                        .copied()
                        .unwrap_or(0)
                });
                let mut plane = BitBlock::from_words(lanes) & active;
                if flips.as_mut().and_then(Iterator::next) == Some(true) {
                    plane ^= active;
                }
                planes.push(plane);
            }
            tile_start = tile_end;
        }
        Ok(())
    }
}

/// Row slices of every 64-shot block of `records`; rows past `shot_count` stay empty and read
/// as zero.
fn block_rows<'a>(
    records: PackedShotBatchView<'a>,
    shot_count: usize,
) -> DetectionResult<Vec<[&'a [u64]; WORD_BITS]>> {
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(shot_count.div_ceil(WORD_BITS))
        .map_err(|error| {
            DetectionError::invalid_result_format(format!("windowed conversion row index: {error}"))
        })?;
    for block_start in (0..shot_count).step_by(WORD_BITS) {
        let mut rows: [&[u64]; WORD_BITS] = [&[]; WORD_BITS];
        for (offset, row) in rows.iter_mut().enumerate() {
            let shot = block_start + offset;
            if shot < shot_count {
                *row = records.shot(shot).map_err(input_error)?.words();
            }
        }
        blocks.push(rows);
    }
    Ok(blocks)
}

fn input_error(error: stab_records::FormatError) -> DetectionError {
    DetectionError::invalid_result_format(error.to_string())
}
