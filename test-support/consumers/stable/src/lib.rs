//! Stable external-consumer fixture for the extracted component stack.

use std::error::Error;

use stab_algebra::{CliffordString, SingleQubitClifford};
use stab_analysis::circuit_without_tags;
use stab_bits::BitVec;
use stab_decoder::{
    DecodeBatchSummary, DecodeCancellation, DecodeSessionFailure, DecoderInputBatchView,
    DecoderLayout, DecoderModelView, DecoderSession, ValidatedDecodeBatch, decode_batch,
};
use stab_engine::SamplingCompiler;
use stab_model::{Circuit, DetectorErrorModel};
use stab_records::{
    BitPlane64Batch, MeasurementBatchView, MeasurementCodecSink, MeasurementSink, MeasurementWidth,
    ObservablePredictionBatch, PackedShotBatch, RecordFormat, SampleFormat,
    read_measurement_records,
};

const STRICT_ZERO_ONE_FIXTURE: &[u8] = b"101001\n010110\n111000\n";

pub fn exercise_stable_components(
) -> Result<(usize, usize, Vec<u8>, Vec<Vec<bool>>), Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaa55; 5]);
    left.xor_assign(&right.as_bitslice())?;

    let mut clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 257))?;
    let phase = CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, 257))?;
    clifford.right_multiply_in_place(&phase)?;

    let circuit = Circuit::from_stim_str("M 0\n")?;
    let stripped = circuit_without_tags(&circuit);
    let _plan = SamplingCompiler::new().compile(&stripped)?;

    let records = read_measurement_records(STRICT_ZERO_ONE_FIXTURE, SampleFormat::ZeroOne, 6)?;
    let shot_major = PackedShotBatch::from_records(&records, 6)?;
    let bit_planes = BitPlane64Batch::from_shot_major(shot_major.view())?;
    let round_trip = bit_planes.to_shot_major()?.to_records()?;
    if round_trip != records {
        return Err(std::io::Error::other("typed record layout conversion changed bits").into());
    }

    let mut sink = MeasurementCodecSink::try_new(RecordFormat::B8, MeasurementWidth::new(6))?;
    sink.write_batch(MeasurementBatchView::from_bit_planes(bit_planes.view()))?;
    let encoded = sink.into_bytes()?;

    let dem = DetectorErrorModel::from_dem_str("error(0.25) D5 L0\n")?;
    let model = DecoderModelView::try_new(&dem)?;
    let mut decoder = FirstDetectorSession {
        layout: model.layout(),
    };
    let mut predictions = ObservablePredictionBatch::zeros(
        shot_major.shot_count(),
        model.layout().correction_width(),
    )?;
    decode_batch(
        &mut decoder,
        DecoderInputBatchView::from_detectors(shot_major.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )?;
    let predictions = predictions.records().to_records()?;

    Ok((left.len(), shot_major.shot_count(), encoded, predictions))
}

#[derive(Debug)]
struct FirstDetectorSession {
    layout: DecoderLayout,
}

impl DecoderSession for FirstDetectorSession {
    type Error = std::io::Error;

    fn layout(&self) -> DecoderLayout {
        self.layout
    }

    fn decode_validated_batch(
        &mut self,
        mut batch: ValidatedDecodeBatch<'_, '_>,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodeBatchSummary, DecodeSessionFailure<Self::Error>> {
        let requested = batch.shot_count();
        let mut completed = 0;
        while completed < requested {
            if cancellation.is_cancelled() {
                return Ok(DecodeBatchSummary::cancelled(requested, completed));
            }
            let prediction = batch.detector(completed, 0).ok_or_else(|| {
                DecodeSessionFailure::new(
                    std::io::Error::other("validated detector bit is missing"),
                    completed,
                )
            })?;
            batch
                .set_prediction(completed, 0, prediction)
                .map_err(|error| {
                    DecodeSessionFailure::new(std::io::Error::other(error), completed)
                })?;
            completed += 1;
        }
        Ok(DecodeBatchSummary::completed(requested))
    }
}

#[cfg(test)]
mod tests {
    use super::{STRICT_ZERO_ONE_FIXTURE, exercise_stable_components};
    use stab_records::{SampleFormat, read_measurement_records};

    #[test]
    fn stable_records_parse_convert_and_encode_without_the_facade()
    -> Result<(), Box<dyn std::error::Error>> {
        let (bit_len, shot_count, encoded, predictions) = exercise_stable_components()?;

        assert_eq!(bit_len, 257);
        assert_eq!(shot_count, 3);
        assert_eq!(encoded, [0x25, 0x1a, 0x07]);
        assert_eq!(
            predictions,
            vec![vec![true], vec![false], vec![true]]
        );

        let unterminated = &STRICT_ZERO_ONE_FIXTURE[..STRICT_ZERO_ONE_FIXTURE.len() - 1];
        assert!(
            read_measurement_records(unterminated, SampleFormat::ZeroOne, 6).is_err(),
            "strict 01 parsing must reject an unterminated final record"
        );
        Ok(())
    }
}
