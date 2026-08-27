//! Scalar external-consumer fixture for the Stab facade.

use std::convert::Infallible;
use std::error::Error;

use stab_core::execution::{RandomPolicy, SamplingCompiler, Seed, ShotCount};
use stab_core::{
    Circuit, DecoderLayout, DetectorWidth, MeasurementBatchView, MeasurementSink, ObservableWidth,
};

#[derive(Default)]
struct CountingSink {
    shots: usize,
    set_bits: usize,
}

impl MeasurementSink for CountingSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.shots += batch.shot_count();
        for shot in 0..batch.shot_count() {
            for bit in 0..batch.width().get() {
                self.set_bits += usize::from(batch.get(shot, bit) == Some(true));
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn exercise_scalar_facade() -> Result<(usize, usize, usize, usize), Box<dyn Error>> {
    let circuit = Circuit::from_stim_str("M 0 1\nDETECTOR rec[-1]\n")?;
    let unchanged = stab_core::analysis::circuit_with_inlined_feedback(&circuit)?;
    if unchanged != circuit {
        return Err(std::io::Error::other("feedback-free circuit changed").into());
    }

    let decoder_layout = stab_core::decoder::DecoderLayout::new(
        DetectorWidth::new(1),
        ObservableWidth::new(0),
    );
    let _: DecoderLayout = decoder_layout;

    let plan = SamplingCompiler::new().compile(&circuit)?;
    let mut session = plan.session(RandomPolicy::Seeded(Seed::new(7)))?;
    let mut sink = CountingSink::default();
    session.run(ShotCount::new(3), &mut sink)?;

    Ok((
        plan.measurement_width().get(),
        decoder_layout.detector_width().get(),
        sink.shots,
        sink.set_bits,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_composes_analysis_decoder_and_execution_namespaces() {
        assert_eq!(exercise_scalar_facade().unwrap(), (2, 1, 3, 0));
    }
}
