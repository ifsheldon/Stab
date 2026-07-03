use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, Pauli, QubitId, Target,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingDetectorOptions {
    pub ignore_non_deterministic_measurements: bool,
}

pub fn missing_detectors(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> CircuitResult<Circuit> {
    let mut finder = BasicMissingDetectorFinder {
        options,
        measurements: Vec::new(),
        known_basis: BTreeMap::new(),
    };
    finder.process_circuit(circuit)?;
    finder.build_output()
}

struct BasicMissingDetectorFinder {
    options: MissingDetectorOptions,
    measurements: Vec<MeasurementInfo>,
    known_basis: BTreeMap<QubitId, Pauli>,
}

struct MeasurementInfo {
    deterministic: bool,
    covered: bool,
}

impl BasicMissingDetectorFinder {
    fn process_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.process_instruction(instruction)?,
                CircuitItem::RepeatBlock(_) => {
                    return Err(CircuitError::invalid_detector_error_model(
                        "basic missing-detector analysis does not support repeat blocks",
                    ));
                }
            }
        }
        Ok(())
    }

    fn process_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "R" => self.process_reset(instruction, Pauli::Z),
            "RX" => self.process_reset(instruction, Pauli::X),
            "RY" => self.process_reset(instruction, Pauli::Y),
            "M" | "MR" => self.process_measurement(instruction, Pauli::Z),
            "MX" | "MRX" => self.process_measurement(instruction, Pauli::X),
            "MY" | "MRY" => self.process_measurement(instruction, Pauli::Y),
            "DETECTOR" => self.process_detector(instruction),
            "OBSERVABLE_INCLUDE" => Err(CircuitError::invalid_detector_error_model(
                "basic missing-detector analysis does not support observable interactions",
            )),
            "TICK" => Ok(()),
            "MPP" | "MXX" | "MYY" | "MZZ" => Err(CircuitError::invalid_detector_error_model(
                "basic missing-detector analysis does not support Pauli-product measurements",
            )),
            name => Err(CircuitError::invalid_detector_error_model(format!(
                "basic missing-detector analysis does not support gate {name}"
            ))),
        }
    }

    fn process_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: Pauli,
    ) -> CircuitResult<()> {
        for qubit in instruction_qubits(instruction)? {
            self.known_basis.insert(qubit, basis);
        }
        Ok(())
    }

    fn process_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: Pauli,
    ) -> CircuitResult<()> {
        for qubit in instruction_qubits(instruction)? {
            let known_basis = self.known_basis.get(&qubit).copied().or_else(|| {
                (!self.options.ignore_non_deterministic_measurements).then_some(Pauli::Z)
            });
            let deterministic = known_basis.is_some_and(|known| known == basis);
            self.measurements.push(MeasurementInfo {
                deterministic,
                covered: false,
            });
            self.known_basis.insert(qubit, basis);
        }
        Ok(())
    }

    fn process_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let mut covered_records = BTreeSet::new();
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            let index = self.absolute_record_index(offset)?;
            if !covered_records.insert(index) {
                covered_records.remove(&index);
            }
        }
        if covered_records.len() > 1 {
            return Err(CircuitError::invalid_detector_error_model(
                "basic missing-detector analysis does not support multi-record detector rows",
            ));
        }
        for index in covered_records {
            let Some(measurement) = self.measurements.get_mut(index) else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "detector target resolved to missing measurement index {index}"
                )));
            };
            measurement.covered = true;
        }
        Ok(())
    }

    fn absolute_record_index(&self, offset: MeasureRecordOffset) -> CircuitResult<usize> {
        let current = i64::try_from(self.measurements.len()).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement count does not fit i64 during missing-detector analysis",
            )
        })?;
        let index = current
            .checked_add(i64::from(offset.get()))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "measurement record offset overflowed during missing-detector analysis",
                )
            })?;
        if index < 0 || index >= current {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record target rec[{}] is outside missing-detector analysis history",
                offset.get()
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement record index does not fit usize during missing-detector analysis",
            )
        })
    }

    fn build_output(&self) -> CircuitResult<Circuit> {
        let mut result = Circuit::new();
        let total = self.measurements.len();
        for (index, measurement) in self.measurements.iter().enumerate() {
            if measurement.covered {
                continue;
            }
            if !measurement.deterministic {
                continue;
            }
            result.append_instruction(CircuitInstruction::new(
                Gate::from_name("DETECTOR")?,
                Vec::new(),
                vec![Target::measurement_record(relative_offset(index, total)?)],
                None,
            )?);
        }
        Ok(result)
    }
}

fn instruction_qubits(instruction: &CircuitInstruction) -> CircuitResult<Vec<QubitId>> {
    instruction
        .targets()
        .iter()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })
        })
        .collect()
}

fn relative_offset(index: usize, total: usize) -> CircuitResult<MeasureRecordOffset> {
    let index = i64::try_from(index).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement index does not fit i64 during missing-detector output",
        )
    })?;
    let total = i64::try_from(total).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement count does not fit i64 during missing-detector output",
        )
    })?;
    let offset = index.checked_sub(total).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "relative detector offset overflowed during missing-detector output",
        )
    })?;
    MeasureRecordOffset::try_new(i32::try_from(offset).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "relative detector offset {offset} does not fit i32"
        ))
    })?)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "missing-detector parity tests use exact circuit text for compact diagnostics"
    )]

    use super::*;

    fn missing(text: &str, ignore_non_deterministic_measurements: bool) -> String {
        let circuit = Circuit::from_stim_str(text).unwrap();
        missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements,
            },
        )
        .unwrap()
        .to_stim_string()
    }

    #[test]
    fn missing_detectors_basic() {
        assert_eq!(missing("", false), "");
        assert_eq!(missing("R 0\nM 0\nDETECTOR rec[-1]\n", false), "");
        assert_eq!(
            missing("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false),
            ""
        );
        assert_eq!(missing("R 0\nM 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("M 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("M 0\n", true), "");
        assert_eq!(
            missing("R 0 1\nM 0 1\nDETECTOR rec[-1]\n", false),
            "DETECTOR rec[-2]\n"
        );
        assert_eq!(
            missing("M 0\nDETECTOR rec[-1] rec[-1]\n", false),
            "DETECTOR rec[-1]\n"
        );
        assert_eq!(missing("MX 0\n", false), "");
    }

    #[test]
    fn missing_detectors_rejects_product_measurements() {
        let circuit = Circuit::from_stim_str("MPP Z0*Z1\n").unwrap();
        let error = missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("Pauli-product measurements"));
    }

    #[test]
    fn missing_detectors_basic_reset_and_measurement_aliases() {
        assert_eq!(missing("RX 0\nMX 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("RY 0\nMY 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("RX 0\nMY 0\n", false), "");
        assert_eq!(missing("RX 0\nMY 0\n", true), "");
        assert_eq!(missing("MR 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("MR 0\n", true), "");
    }

    #[test]
    fn missing_detectors_rejects_multi_record_detector_rows() {
        let circuit = Circuit::from_stim_str("R 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n").unwrap();
        let error = missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("multi-record detector rows"));
    }

    #[test]
    fn missing_detectors_rejects_observable_interactions() {
        let circuit = Circuit::from_stim_str("M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
        let error = missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: true,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("observable interactions"));
    }
}
