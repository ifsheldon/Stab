//! External-consumer fixture for the Stab facade.

use std::convert::Infallible;
use std::error::Error;

use stab_core::execution::{
    RandomPolicy, SamplingCompiler, SamplingSessionOptions, Seed, ShotCount,
};
use stab_core::{
    BitIndex, Circuit, CliffordString, DecoderLayout, DetectorWidth, MeasurementBatchView,
    MeasurementSink, ObservableWidth, ShotIndex, SingleQubitClifford,
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
                self.set_bits +=
                    usize::from(batch.get(ShotIndex(shot), BitIndex(bit)) == Some(true));
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn exercise_facade_namespaces() -> Result<(usize, usize, usize, usize), Box<dyn Error>> {
    let circuit = Circuit::from_stim_str("M 0 1\nDETECTOR rec[-1]\n")?;
    let unchanged =
        stab_core::analysis::circuit_with_inlined_feedback(&circuit, &Default::default())?;
    if unchanged != circuit {
        return Err(std::io::Error::other("feedback-free circuit changed").into());
    }

    let decoder_layout =
        stab_core::decoder::DecoderLayout::new(DetectorWidth::new(1), ObservableWidth::new(0));
    let _: DecoderLayout = decoder_layout;

    let plan = SamplingCompiler::default().compile(&circuit)?;
    let mut session = plan.session(&SamplingSessionOptions {
        random: RandomPolicy::Seeded(Seed::new(7)),
        ..SamplingSessionOptions::default()
    })?;
    let mut sink = CountingSink::default();
    session.run(ShotCount::new(3), &mut sink)?;

    Ok((
        plan.measurement_width().get(),
        decoder_layout.detector_width().get(),
        sink.shots,
        sink.set_bits,
    ))
}

pub fn exercise_facade_clifford_kernel() -> Result<usize, Box<dyn Error>> {
    let mut clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 257))?;
    let phase = CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, 257))?;
    clifford.right_multiply_in_place(&phase)?;
    Ok(clifford.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_composes_analysis_decoder_and_execution_namespaces() {
        assert_eq!(exercise_facade_namespaces().unwrap(), (2, 1, 3, 0));
    }

    #[test]
    fn facade_executes_clifford_kernel_work() {
        assert_eq!(exercise_facade_clifford_kernel().unwrap(), 257);
    }
}
