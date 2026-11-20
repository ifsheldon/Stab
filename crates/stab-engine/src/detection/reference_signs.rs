use stab_bits::{BitSlice, BitVec};
use stab_model::Circuit;

use super::buffers::resource_amount;
use super::error::DetectionResourceLimitError;
use super::{
    DetectionConversionLimits, DetectionError, DetectionRecordBuffer,
    PreparedMeasurementToDetection,
};

/// Detector and observable parities implied by a circuit's noiseless reference sample, each
/// packed one bit per detector or observable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitReferenceSigns {
    detector_signs: BitVec,
    observable_signs: BitVec,
}

impl CircuitReferenceSigns {
    /// Detector signs in circuit declaration order.
    pub fn detector_signs(&self) -> BitSlice<'_> {
        self.detector_signs.as_bitslice()
    }

    /// Observable signs indexed by observable id, including unused ids below the maximum id.
    pub fn observable_signs(&self) -> BitSlice<'_> {
        self.observable_signs.as_bitslice()
    }

    /// Bytes the packed signs of `detectors` and `observables` occupy.
    fn storage_bytes(detectors: usize, observables: usize) -> Result<u64, DetectionError> {
        let words = resource_amount(
            detectors.div_ceil(u64::BITS as usize),
            "detector reference-sign words",
        )?
        .checked_add(resource_amount(
            observables.div_ceil(u64::BITS as usize),
            "observable reference-sign words",
        )?);
        words
            .and_then(|words| words.checked_mul(size_of::<u64>() as u64))
            .ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "reference-sign storage accounting overflowed",
                )
            })
    }
}

/// Computes detector and observable signs using the noiseless all-zero-sweep reference sample.
///
/// Detection-plan work is admitted with [`DetectionConversionLimits::default`]. Use
/// [`circuit_reference_signs_with_limits`] when the caller owns a different resource policy.
pub fn circuit_reference_signs(circuit: &Circuit) -> Result<CircuitReferenceSigns, DetectionError> {
    circuit_reference_signs_with_limits(circuit, DetectionConversionLimits::default())
}

/// Computes reference signs after applying the caller's detection-conversion resource limits.
///
/// The packed signs are admitted against the budget together with the compiled conversion that
/// computes them.
pub fn circuit_reference_signs_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> Result<CircuitReferenceSigns, DetectionError> {
    let budget = limits.memory_budget();
    let prepared = PreparedMeasurementToDetection::compile_with_limits(
        circuit,
        crate::ReferenceSampleMode::UseReferenceSample,
        limits,
    )?;
    let total_bytes = prepared
        .compiled_bytes()?
        .checked_add(CircuitReferenceSigns::storage_bytes(
            prepared.detector_count(),
            prepared.observable_count(),
        )?)
        .ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "reference-sign storage accounting overflowed",
            )
        })?;
    if !budget.admits(u128::from(total_bytes)) {
        return Err(DetectionResourceLimitError::detection_compiled_bytes(
            total_bytes,
            budget.bytes(),
        )
        .into());
    }
    let mut record: DetectionRecordBuffer = prepared.try_reusable_detection_record()?;
    prepared
        .plan
        .reference_signs_into(prepared.reference_sample.sample(), &mut record)?;
    Ok(CircuitReferenceSigns {
        detector_signs: try_pack(&record.detectors, "detector reference signs")?,
        observable_signs: try_pack(&record.observables, "observable reference signs")?,
    })
}

/// Packs `bits` one bit per entry, allocating the words fallibly.
fn try_pack(bits: &[bool], context: &str) -> Result<BitVec, DetectionError> {
    let mut packed = BitVec::try_zeros(bits.len()).map_err(|error| {
        DetectionError::storage_allocation(format!("unable to allocate {context}: {error}"))
    })?;
    for (word, chunk) in packed
        .words_mut()
        .iter_mut()
        .zip(bits.chunks(u64::BITS as usize))
    {
        *word = chunk
            .iter()
            .rev()
            .fold(0_u64, |word, bit| (word << 1) | u64::from(*bit));
    }
    Ok(packed)
}
