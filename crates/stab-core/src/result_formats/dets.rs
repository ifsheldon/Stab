use crate::{CircuitError, CircuitResult};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DetsResultType {
    Measurement,
    Detector,
    Observable,
}

impl DetsResultType {
    pub const fn prefix(self) -> u8 {
        match self {
            Self::Measurement => b'M',
            Self::Detector => b'D',
            Self::Observable => b'L',
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DetsToken {
    result_type: DetsResultType,
    index: usize,
}

impl DetsToken {
    pub const fn new(result_type: DetsResultType, index: usize) -> Self {
        Self { result_type, index }
    }

    pub const fn result_type(self) -> DetsResultType {
        self.result_type
    }

    pub const fn index(self) -> usize {
        self.index
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DetsLayout {
    measurements: usize,
    detectors: usize,
    observables: usize,
    total_bits: usize,
}

impl DetsLayout {
    pub fn try_new(
        measurements: usize,
        detectors: usize,
        observables: usize,
    ) -> CircuitResult<Self> {
        let total_bits = measurements
            .checked_add(detectors)
            .and_then(|value| value.checked_add(observables))
            .ok_or_else(|| {
                CircuitError::invalid_result_format("DETS layout total width overflowed")
            })?;
        Ok(Self {
            measurements,
            detectors,
            observables,
            total_bits,
        })
    }

    pub const fn measurement_only(measurements: usize) -> Self {
        Self {
            measurements,
            detectors: 0,
            observables: 0,
            total_bits: measurements,
        }
    }

    pub const fn measurements(self) -> usize {
        self.measurements
    }

    pub const fn detectors(self) -> usize {
        self.detectors
    }

    pub const fn observables(self) -> usize {
        self.observables
    }

    pub const fn total_bits(self) -> usize {
        self.total_bits
    }

    pub(crate) fn resolve(self, result_type: DetsResultType, index: usize) -> CircuitResult<usize> {
        let (offset, count) = match result_type {
            DetsResultType::Measurement => (0, self.measurements),
            DetsResultType::Detector => (self.measurements, self.detectors),
            DetsResultType::Observable => (
                self.measurements
                    .checked_add(self.detectors)
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format(
                            "DETS observable offset overflowed layout width",
                        )
                    })?,
                self.observables,
            ),
        };
        if index >= count {
            return Err(CircuitError::invalid_result_format(format!(
                "DETS token {}{index} exceeds namespace width {count}",
                char::from(result_type.prefix())
            )));
        }
        offset.checked_add(index).ok_or_else(|| {
            CircuitError::invalid_result_format("DETS token offset overflowed layout width")
        })
    }
}

pub fn read_dets_records(input: &[u8], layout: DetsLayout) -> CircuitResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    crate::result_text::for_each_dets_tokens(input, layout, |tokens| {
        let mut record = vec![false; layout.total_bits()];
        for token in tokens {
            let index = layout.resolve(token.result_type(), token.index())?;
            let bit = record.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "DETS token resolved beyond the layout's total width",
                )
            })?;
            *bit = true;
        }
        records.push(record);
        Ok(())
    })?;
    Ok(records)
}
