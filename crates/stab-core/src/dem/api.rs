use crate::{CircuitError, CircuitResult};

use super::{DemInstructionKind, DemItem, DetectorErrorModel};

impl DetectorErrorModel {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn append_from_dem_text(&mut self, input: &str) -> CircuitResult<()> {
        let mut parsed = Self::from_dem_str(input)?;
        self.items.append(&mut parsed.items);
        Ok(())
    }

    pub fn without_tags(&self) -> Self {
        Self {
            items: self.items.iter().map(DemItem::without_tags).collect(),
        }
    }

    pub fn final_coordinate_shift(&self) -> CircuitResult<Vec<f64>> {
        coordinate_shift_of(self)
    }
}

impl DemItem {
    fn without_tags(&self) -> Self {
        match self {
            Self::Instruction(instruction) => {
                let mut instruction = instruction.clone();
                instruction.tag = None;
                Self::Instruction(instruction)
            }
            Self::RepeatBlock(repeat) => {
                let mut repeat = repeat.clone();
                repeat.tag = None;
                repeat.body = repeat.body.without_tags();
                Self::RepeatBlock(repeat)
            }
        }
    }
}

fn coordinate_shift_of(model: &DetectorErrorModel) -> CircuitResult<Vec<f64>> {
    let mut shift = Vec::new();
    apply_coordinate_shift_of(model, &mut shift)?;
    Ok(shift)
}

fn apply_coordinate_shift_of(
    model: &DetectorErrorModel,
    shift: &mut Vec<f64>,
) -> CircuitResult<()> {
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => {
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    add_coordinate_shift_mul(shift, instruction.args(), 1.0)?;
                }
            }
            DemItem::RepeatBlock(repeat) => {
                let body_shift = coordinate_shift_of(repeat.body())?;
                add_coordinate_shift_mul(shift, &body_shift, repeat.repeat_count().get() as f64)?;
            }
        }
    }
    Ok(())
}

fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> CircuitResult<()> {
    if shift.len() < delta.len() {
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift.get_mut(index).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("coordinate shift dimension missing")
        })?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(CircuitError::invalid_detector_error_model(
                "coordinate shift overflowed",
            ));
        }
    }
    Ok(())
}
