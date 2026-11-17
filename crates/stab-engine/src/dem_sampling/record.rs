//! Sampled-error recording layered on the detector stream.
//!
//! A recording run draws exactly the detector streams of a detector-only run, in the same order,
//! so its detector and observable planes are identical. Errors that the detector walk samples
//! directly are recorded from their own draws. Errors it skips or folds are filled from two
//! stream families derived from the run seed: errors that cannot fire stay clear, deterministic
//! errors are set, and the copies of an error inside a folded zero-shift repeat are sampled
//! independently and then conditioned on the folded parity the detector stream drew.

use super::bit_plane::{
    DetectionStream, detector_shift_overflow_error, indexed_range_error, sample_block, sample_error,
};
use super::program::{
    DemSampleBlock, DemSampleError, DemSampleOperation, DemSampleRepeat, odd_parity_probability,
};
use super::{DemError, DemResult};
use crate::bernoulli::{
    INDEXED_BLOCK_SHOTS, INDEXED_BLOCK_WORDS, ShotUniforms, SparseBernoulli, derived_seed,
    sample_indexed_range_into,
};

/// Domain of the streams that sample each copy of a folded error independently.
const COPY_STREAM_DOMAIN: u64 = 0x6A09_E667_F3BC_C908;
/// Domain of the per-shot streams that condition folded copies on their drawn parity.
const SHOT_STREAM_DOMAIN: u64 = 0xBB67_AE85_84CA_A73B;

const WORD_BITS: usize = u64::BITS as usize;

/// Samples `block` as [`sample_block`] does and records every sampled error into `error_planes`.
pub(super) fn record_block(
    block: &DemSampleBlock,
    stream: DetectionStream<'_, '_>,
    shot_count: usize,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_planes: &mut [u64],
    stream_cursor: &mut usize,
) -> DemResult<()> {
    let seed = stream.seed;
    let mut recorder = Recorder {
        stream,
        shot_count,
        word_count: shot_count.div_ceil(WORD_BITS),
        copy_seed: derived_seed(seed, COPY_STREAM_DOMAIN),
        shot_seed: derived_seed(seed, SHOT_STREAM_DOMAIN),
        detector_planes,
        observable_planes,
        error_planes,
        stream_cursor,
    };
    recorder.record(block, 0, 0)
}

struct Recorder<'a, 'p> {
    stream: DetectionStream<'a, 'p>,
    shot_count: usize,
    word_count: usize,
    copy_seed: u64,
    shot_seed: u64,
    detector_planes: &'a mut [u64],
    observable_planes: &'a mut [u64],
    error_planes: &'a mut [u64],
    stream_cursor: &'a mut usize,
}

/// How the detector walk treats the copies of one repeat level inside a skipped or folded repeat.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LevelFold {
    /// A stochastic zero-shift repeat whose copies the detector walk folds into one parity error.
    Folded,
    /// A deterministic zero-shift repeat with an odd count, applied once.
    Once,
    /// A deterministic zero-shift repeat with an even count, which cancels.
    Cancelled,
}

/// One repeat level enclosing an error inside a skipped or folded repeat, innermost first.
struct Level<'a> {
    count: u64,
    /// Error ordinals between consecutive iterations of this level.
    stride: usize,
    /// Copies contributed by all enclosing levels, so the weight of this level's iteration index.
    outer_copies: u64,
    fold: LevelFold,
    parent: Option<&'a Level<'a>>,
}

impl<'a> Level<'a> {
    fn new(
        repeat: &DemSampleRepeat,
        fold: LevelFold,
        parent: Option<&'a Level<'a>>,
    ) -> DemResult<Self> {
        Ok(Self {
            count: repeat.repeat_count,
            stride: repeat.body.error_count,
            outer_copies: parent.map_or(Some(1), Level::copies).ok_or_else(|| {
                DemError::invalid_sampler_compilation("DEM sampled-error copy count overflowed")
            })?,
            fold,
            parent,
        })
    }

    fn nested_fold(repeat: &DemSampleRepeat) -> LevelFold {
        if repeat.body.direct_sample_has_stochastic_error {
            LevelFold::Folded
        } else if repeat.repeat_count.is_multiple_of(2) {
            LevelFold::Cancelled
        } else {
            LevelFold::Once
        }
    }

    fn copies(&self) -> Option<u64> {
        self.count.checked_mul(self.outer_copies)
    }
}

/// Every unrolled copy of one compiled error inside a skipped or folded repeat.
#[derive(Clone, Copy)]
struct Copies<'a> {
    level: &'a Level<'a>,
    /// Ordinal of the copy in the first iteration of every enclosing level.
    first_ordinal: usize,
    count: u64,
}

impl<'a> Copies<'a> {
    fn new(level: &'a Level<'a>, first_ordinal: usize) -> DemResult<Self> {
        Ok(Self {
            level,
            first_ordinal,
            count: level.copies().ok_or_else(|| {
                DemError::invalid_sampler_compilation("DEM sampled-error copy count overflowed")
            })?,
        })
    }

    /// The error ordinal of copy `index`, whose innermost level index is most significant.
    fn ordinal(self, index: u64) -> DemResult<usize> {
        let mut ordinal = self.first_ordinal;
        let mut rest = index;
        let mut level = Some(self.level);
        while let Some(current) = level {
            let iteration = rest / current.outer_copies;
            rest %= current.outer_copies;
            ordinal = usize::try_from(iteration)
                .ok()
                .and_then(|iteration| iteration.checked_mul(current.stride))
                .and_then(|offset| ordinal.checked_add(offset))
                .ok_or_else(ordinal_overflow_error)?;
            level = current.parent;
        }
        Ok(ordinal)
    }

    /// The probability of the folded parity error the detector walk draws for these copies of an
    /// error of `probability`, or `None` when it draws none.
    ///
    /// This repeats, level by level, the folding and the zero-probability pruning of the
    /// compiler, so it selects exactly the folded errors it kept.
    fn folded_probability(self, probability: f64) -> Option<f64> {
        let mut folded = probability;
        let mut level = Some(self.level);
        while let Some(current) = level {
            match current.fold {
                LevelFold::Cancelled => return None,
                LevelFold::Once => {}
                LevelFold::Folded => {
                    folded = odd_parity_probability(folded, current.count);
                    if folded <= 0.0 {
                        return None;
                    }
                }
            }
            level = current.parent;
        }
        Some(folded)
    }
}

impl Recorder<'_, '_> {
    fn record(
        &mut self,
        block: &DemSampleBlock,
        detector_shift: u64,
        first_ordinal: usize,
    ) -> DemResult<()> {
        let mut ordinal = first_ordinal;
        for operation in &block.operations {
            match operation {
                DemSampleOperation::Error(error) => {
                    self.stream.poll.tick()?;
                    let plane = error_plane(self.error_planes, ordinal, self.word_count)?;
                    sample_error(
                        error,
                        detector_shift,
                        self.stream.seed,
                        self.stream.range,
                        self.detector_planes,
                        self.observable_planes,
                        Some(plane),
                        self.stream_cursor,
                    )?;
                    ordinal = ordinal.checked_add(1).ok_or_else(ordinal_overflow_error)?;
                }
                DemSampleOperation::Repeat(repeat) => {
                    self.record_repeat(repeat, detector_shift, ordinal)?;
                    ordinal = ordinal
                        .checked_add(repeat_span(repeat)?)
                        .ok_or_else(ordinal_overflow_error)?;
                }
            }
        }
        Ok(())
    }

    /// Records one repeat, making the detector-stream decisions of [`sample_block`].
    fn record_repeat(
        &mut self,
        repeat: &DemSampleRepeat,
        detector_shift: u64,
        first_ordinal: usize,
    ) -> DemResult<()> {
        if repeat.body.direct_sample_effect_count == 0 {
            // No error of the repeat can fire, so its error planes stay clear.
            return Ok(());
        }
        let shift = detector_shift
            .checked_add(repeat.start_detector_shift)
            .ok_or_else(detector_shift_overflow_error)?;
        if repeat.body.detector_shift == 0 && !repeat.body.direct_sample_has_stochastic_error {
            let fold = if repeat.repeat_count.is_multiple_of(2) {
                LevelFold::Cancelled
            } else {
                sample_block(
                    &repeat.body,
                    shift,
                    self.stream.reborrow(),
                    self.detector_planes,
                    self.observable_planes,
                    self.stream_cursor,
                )?;
                LevelFold::Once
            };
            let level = Level::new(repeat, fold, None)?;
            return self.fill_leaves(&repeat.body, &level, first_ordinal, shift, None, &mut 0);
        }
        if let Some(folded_errors) = repeat.folded_zero_shift_errors.as_deref() {
            let level = Level::new(repeat, LevelFold::Folded, None)?;
            let mut kept = 0_usize;
            self.fill_leaves(
                &repeat.body,
                &level,
                first_ordinal,
                shift,
                Some(folded_errors),
                &mut kept,
            )?;
            if kept != folded_errors.len() {
                return Err(DemError::invalid_sampler_compilation(format!(
                    "DEM sampled-error recording matched {kept} of {} folded errors",
                    folded_errors.len()
                )));
            }
            return Ok(());
        }
        let mut shift = shift;
        let mut ordinal = first_ordinal;
        for _ in 0..repeat.repeat_count {
            self.stream.poll.tick()?;
            self.record(&repeat.body, shift, ordinal)?;
            shift = shift
                .checked_add(repeat.body.detector_shift)
                .ok_or_else(detector_shift_overflow_error)?;
            ordinal = ordinal
                .checked_add(repeat.body.error_count)
                .ok_or_else(ordinal_overflow_error)?;
        }
        Ok(())
    }

    /// Fills every copy of every error of a deterministic or folded zero-shift repeat body.
    ///
    /// With `folded_errors`, each error whose copies the detector walk folded first draws that
    /// folded error at its position in the detector stream.
    fn fill_leaves(
        &mut self,
        block: &DemSampleBlock,
        level: &Level<'_>,
        first_ordinal: usize,
        detector_shift: u64,
        folded_errors: Option<&[DemSampleError]>,
        kept: &mut usize,
    ) -> DemResult<()> {
        let mut ordinal = first_ordinal;
        for operation in &block.operations {
            match operation {
                DemSampleOperation::Error(error) => {
                    let copies = Copies::new(level, ordinal)?;
                    self.fill_error_copies(error, copies, detector_shift, folded_errors, kept)?;
                    ordinal = ordinal.checked_add(1).ok_or_else(ordinal_overflow_error)?;
                }
                DemSampleOperation::Repeat(nested) => {
                    let nested_level = Level::new(nested, Level::nested_fold(nested), Some(level))?;
                    self.fill_leaves(
                        &nested.body,
                        &nested_level,
                        ordinal,
                        detector_shift,
                        folded_errors,
                        kept,
                    )?;
                    ordinal = ordinal
                        .checked_add(repeat_span(nested)?)
                        .ok_or_else(ordinal_overflow_error)?;
                }
            }
        }
        Ok(())
    }

    fn fill_error_copies(
        &mut self,
        error: &DemSampleError,
        copies: Copies<'_>,
        detector_shift: u64,
        folded_errors: Option<&[DemSampleError]>,
        kept: &mut usize,
    ) -> DemResult<()> {
        let probability = error.probability();
        let mut target_words = [0_u64; INDEXED_BLOCK_WORDS];
        let target = target_words
            .get_mut(..self.word_count)
            .ok_or_else(window_width_error)?;
        match folded_errors {
            None if probability > 0.0 && probability < 1.0 => {
                return Err(DemError::invalid_sampler_compilation(
                    "deterministic DEM repeat holds a stochastic error",
                ));
            }
            None => {}
            Some(folded_errors) => {
                if let Some(folded_probability) = copies.folded_probability(probability) {
                    let folded = folded_errors
                        .get(*kept)
                        .filter(|folded| folded.probability() == folded_probability)
                        .ok_or_else(|| {
                            DemError::invalid_sampler_compilation(
                                "DEM sampled-error recording diverged from the folded errors",
                            )
                        })?;
                    *kept += 1;
                    self.stream.poll.tick()?;
                    sample_error(
                        folded,
                        detector_shift,
                        self.stream.seed,
                        self.stream.range,
                        self.detector_planes,
                        self.observable_planes,
                        Some(&mut *target),
                        self.stream_cursor,
                    )?;
                }
            }
        }
        if probability <= 0.0 {
            return Ok(());
        }
        if probability >= 1.0 {
            for index in 0..copies.count {
                self.stream.poll.tick()?;
                let plane =
                    error_plane(self.error_planes, copies.ordinal(index)?, self.word_count)?;
                for (word, valid) in plane.iter_mut().zip(valid_words(self.shot_count)) {
                    *word = valid;
                }
            }
            return Ok(());
        }
        self.fill_conditioned_copies(probability, copies, target)
    }

    /// Samples the copies of an error of `probability` conditioned on the odd-parity mask
    /// `target` the detector walk drew for them.
    ///
    /// The copies are first sampled independently. Write `y` for the copies, complemented when
    /// the probability exceeds one half, so each bit of `y` is one with probability
    /// `rho <= 1/2`. Each `y` bit is `c AND u` for a randomization bit `c` of probability
    /// `2 rho` and a fair coin `u`. When some copy is randomized, the parity of `y` is a fair coin
    /// given `c`, and toggling the coin of the last randomized copy maps one parity onto the
    /// other, so it conditions the sample exactly. A zero `y` bit is randomized with probability
    /// `rho / (1 - rho)`, which locates the last randomized copy without materializing `c`. The
    /// empty randomization set has parity even and weight `e = (1 - 2 rho)^N`, so an even target
    /// keeps a nonempty set with probability `1 / (1 + e)` and an odd target redraws an empty one.
    fn fill_conditioned_copies(
        &mut self,
        probability: f64,
        copies: Copies<'_>,
        target: &[u64],
    ) -> DemResult<()> {
        let range = self.stream.range.map_err(indexed_range_error)?;
        let words = self.word_count;
        let complemented = probability > 0.5;
        let rho = if complemented {
            1.0 - probability
        } else {
            probability
        };
        let mut parity_words = [0_u64; INDEXED_BLOCK_WORDS];
        let mut any_words = [0_u64; INDEXED_BLOCK_WORDS];
        let parity = parity_words
            .get_mut(..words)
            .ok_or_else(window_width_error)?;
        let any = any_words.get_mut(..words).ok_or_else(window_width_error)?;
        // The last nonzero `y` copy of each shot, tracked per hit on the sparse branch and
        // otherwise found by a backward scan over only the shots that need it.
        let mut last_nonzero = [0_u64; INDEXED_BLOCK_SHOTS];
        let sparse = SparseBernoulli::new(probability);
        for index in 0..copies.count {
            self.stream.poll.tick()?;
            let ordinal = copies.ordinal(index)?;
            let stream_index = u64::try_from(ordinal).map_err(|_| ordinal_overflow_error())?;
            let plane = error_plane(self.error_planes, ordinal, words)?;
            if let Some(sparse) = &sparse {
                // The sparse branch is below one half, so `y` is the sampled copy itself.
                sparse.try_for_each_hit(self.copy_seed, stream_index, range, |hit| {
                    let word = hit / WORD_BITS;
                    let bit = 1_u64 << (hit % WORD_BITS);
                    *plane.get_mut(word).ok_or_else(window_width_error)? |= bit;
                    *parity.get_mut(word).ok_or_else(window_width_error)? ^= bit;
                    *any.get_mut(word).ok_or_else(window_width_error)? |= bit;
                    *last_nonzero.get_mut(hit).ok_or_else(window_width_error)? = index;
                    Ok::<(), DemError>(())
                })?;
                continue;
            }
            sample_indexed_range_into(
                probability,
                self.copy_seed,
                stream_index,
                range.first_shot(),
                range.shot_count(),
                plane,
            )
            .map_err(indexed_range_error)?;
            for (((parity, any), word), valid) in parity
                .iter_mut()
                .zip(any.iter_mut())
                .zip(plane.iter())
                .zip(valid_words(self.shot_count))
            {
                *parity ^= *word;
                *any |= if complemented { !*word & valid } else { *word };
            }
        }

        let conditioning = Conditioning::new(rho, copies.count);
        let parity_flip = complemented && !copies.count.is_multiple_of(2);
        let mut clear_words = [0_u64; INDEXED_BLOCK_WORDS];
        let mut flip_words = [0_u64; INDEXED_BLOCK_WORDS];
        let mut candidates = [None::<u64>; INDEXED_BLOCK_SHOTS];
        let leaf_key = u64::try_from(copies.first_ordinal).map_err(|_| ordinal_overflow_error())?;
        let mut any_clear = false;
        let mut any_flip = false;
        for (word_index, valid) in valid_words(self.shot_count).enumerate() {
            let target = target.get(word_index).copied().unwrap_or(0) & valid;
            let parity = parity.get(word_index).copied().unwrap_or(0);
            let any = any.get(word_index).copied().unwrap_or(0);
            let odd_target = (target ^ if parity_flip { valid } else { 0 }) & valid;
            let mismatch = (parity ^ target) & valid;
            let mut shots = (any | odd_target) & valid;
            while shots != 0 {
                let bit_index = shots.trailing_zeros() as usize;
                let bit = 1_u64 << bit_index;
                shots &= shots - 1;
                let shot = word_index * WORD_BITS + bit_index;
                let global_shot = range
                    .first_shot()
                    .checked_add(shot as u64)
                    .ok_or_else(ordinal_overflow_error)?;
                let mut uniforms = ShotUniforms::new(self.shot_seed, leaf_key, global_shot);
                if any & bit == 0 {
                    // An odd target over all-zero copies: toggle the last randomized copy, or
                    // redraw an odd sample when no copy is randomized.
                    match conditioning.last_extra_randomized(&mut uniforms) {
                        Some(index) => self.toggle_copy(copies, index, shot)?,
                        None => self.redraw_odd(copies, &conditioning, &mut uniforms, shot)?,
                    }
                    continue;
                }
                if odd_target & bit == 0 && uniforms.unit() < conditioning.clear_probability {
                    *clear_words
                        .get_mut(word_index)
                        .ok_or_else(window_width_error)? |= bit;
                    any_clear = true;
                    continue;
                }
                if mismatch & bit != 0 {
                    *candidates.get_mut(shot).ok_or_else(window_width_error)? =
                        conditioning.last_extra_randomized(&mut uniforms);
                    *flip_words
                        .get_mut(word_index)
                        .ok_or_else(window_width_error)? |= bit;
                    any_flip = true;
                }
            }
        }
        let clear = clear_words.get(..words).ok_or_else(window_width_error)?;
        if any_clear {
            for index in 0..copies.count {
                self.stream.poll.tick()?;
                let plane = error_plane(self.error_planes, copies.ordinal(index)?, words)?;
                for (word, clear) in plane.iter_mut().zip(clear) {
                    if complemented {
                        *word |= *clear;
                    } else {
                        *word &= !*clear;
                    }
                }
            }
        }
        if any_flip {
            if sparse.is_none() {
                self.find_last_nonzero(copies, complemented, flip_words, &mut last_nonzero)?;
            }
            for (word_index, mut word) in flip_words.into_iter().enumerate() {
                while word != 0 {
                    let shot = word_index * WORD_BITS + word.trailing_zeros() as usize;
                    word &= word - 1;
                    let last = *last_nonzero.get(shot).ok_or_else(window_width_error)?;
                    let candidate = candidates.get(shot).copied().flatten();
                    let index = candidate.map_or(last, |candidate| candidate.max(last));
                    self.toggle_copy(copies, index, shot)?;
                }
            }
        }
        Ok(())
    }

    /// Records in `last_nonzero` the last nonzero `y` copy of every shot in `shots`, each of which
    /// has one, scanning copies backward only until every such shot is found.
    fn find_last_nonzero(
        &mut self,
        copies: Copies<'_>,
        complemented: bool,
        shots: [u64; INDEXED_BLOCK_WORDS],
        last_nonzero: &mut [u64; INDEXED_BLOCK_SHOTS],
    ) -> DemResult<()> {
        let mut pending = shots;
        let mut index = copies.count;
        while pending.iter().any(|word| *word != 0) {
            index = index.checked_sub(1).ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampled-error conditioning found no nonzero copy",
                )
            })?;
            self.stream.poll.tick()?;
            let plane = error_plane(self.error_planes, copies.ordinal(index)?, self.word_count)?;
            for (word_index, ((pending, word), valid)) in pending
                .iter_mut()
                .zip(plane.iter())
                .zip(valid_words(self.shot_count))
                .enumerate()
            {
                let nonzero = if complemented { !*word & valid } else { *word };
                let mut found = *pending & nonzero;
                *pending &= !found;
                while found != 0 {
                    let shot = word_index * WORD_BITS + found.trailing_zeros() as usize;
                    found &= found - 1;
                    *last_nonzero.get_mut(shot).ok_or_else(window_width_error)? = index;
                }
            }
        }
        Ok(())
    }

    /// Writes an odd sample of independent copies conditioned on a nonempty randomization set
    /// into a shot whose copies are all zero in `y`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "finite nonnegative geometric positions are compared against the copy count before conversion"
    )]
    fn redraw_odd(
        &mut self,
        copies: Copies<'_>,
        conditioning: &Conditioning,
        uniforms: &mut ShotUniforms,
        shot: usize,
    ) -> DemResult<()> {
        let count = copies.count as f64;
        let log_unrandomized = conditioning.log_unrandomized;
        // The first randomized copy, truncated to the copies.
        let first = ((-uniforms.unit() * conditioning.randomized_probability).ln_1p()
            / log_unrandomized)
            .floor();
        let mut position = if first.is_finite() && first < count {
            first as u64
        } else {
            copies.count - 1
        };
        let mut odd = false;
        loop {
            if uniforms.bit() {
                self.toggle_copy(copies, position, shot)?;
                odd = !odd;
            }
            let gap = (uniforms.open_unit().ln() / log_unrandomized).floor();
            let next = position as f64 + 1.0 + gap;
            if !next.is_finite() || next >= count {
                break;
            }
            self.stream.poll.tick()?;
            position = next as u64;
        }
        if !odd {
            self.toggle_copy(copies, position, shot)?;
        }
        Ok(())
    }

    fn toggle_copy(&mut self, copies: Copies<'_>, index: u64, shot: usize) -> DemResult<()> {
        let plane = error_plane(self.error_planes, copies.ordinal(index)?, self.word_count)?;
        *plane
            .get_mut(shot / WORD_BITS)
            .ok_or_else(window_width_error)? ^= 1_u64 << (shot % WORD_BITS);
        Ok(())
    }
}

/// Per-error constants of the parity conditioning for `N` copies of probability `rho <= 1/2`.
struct Conditioning {
    copies: u64,
    /// `ln(1 - rho / (1 - rho))`, or `None` when every zero copy is randomized.
    log_zero_unrandomized: Option<f64>,
    /// `ln(1 - 2 rho)`.
    log_unrandomized: f64,
    /// `1 - (1 - 2 rho)^N`, the probability that some copy is randomized.
    randomized_probability: f64,
    /// `e / (1 + e)` with `e = (1 - 2 rho)^N`.
    clear_probability: f64,
}

impl Conditioning {
    #[allow(
        clippy::cast_precision_loss,
        reason = "copy counts are far below 2^53, and the weight only needs relative precision"
    )]
    fn new(rho: f64, copies: u64) -> Self {
        let log_unrandomized = (-2.0 * rho).ln_1p();
        let log_empty = copies as f64 * log_unrandomized;
        let empty = log_empty.exp();
        Self {
            copies,
            log_zero_unrandomized: (rho < 0.5).then(|| (-(rho / (1.0 - rho))).ln_1p()),
            log_unrandomized,
            randomized_probability: -log_empty.exp_m1(),
            clear_probability: empty / (1.0 + empty),
        }
    }

    /// The last copy, counting down from the end, at which a zero copy would be randomized, or
    /// `None` when no copy is.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "the finite nonnegative geometric gap is compared against the copy count before conversion"
    )]
    fn last_extra_randomized(&self, uniforms: &mut ShotUniforms) -> Option<u64> {
        let Some(log_zero_unrandomized) = self.log_zero_unrandomized else {
            return self.copies.checked_sub(1);
        };
        let gap = (uniforms.open_unit().ln() / log_zero_unrandomized).floor();
        if !gap.is_finite() || gap >= self.copies as f64 {
            return None;
        }
        Some(self.copies - 1 - gap as u64)
    }
}

fn error_plane(
    error_planes: &mut [u64],
    ordinal: usize,
    word_count: usize,
) -> DemResult<&mut [u64]> {
    ordinal
        .checked_mul(word_count)
        .and_then(|start| error_planes.get_mut(start..start.checked_add(word_count)?))
        .ok_or_else(|| {
            DemError::invalid_sampler_compilation(
                "DEM sampled-error plane index escaped compiled storage",
            )
        })
}

/// The valid-shot mask of each word of a `shot_count`-shot window.
fn valid_words(shot_count: usize) -> impl Iterator<Item = u64> {
    (0..shot_count.div_ceil(WORD_BITS)).map(move |word| {
        let remaining = shot_count - word * WORD_BITS;
        if remaining >= WORD_BITS {
            u64::MAX
        } else {
            (1_u64 << remaining) - 1
        }
    })
}

fn repeat_span(repeat: &DemSampleRepeat) -> DemResult<usize> {
    usize::try_from(repeat.repeat_count)
        .ok()
        .and_then(|count| count.checked_mul(repeat.body.error_count))
        .ok_or_else(ordinal_overflow_error)
}

fn ordinal_overflow_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM sampled-error ordinal overflowed")
}

fn window_width_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM sampled-error window exceeds indexed block storage")
}
