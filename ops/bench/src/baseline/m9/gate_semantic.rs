use std::hint::black_box;

use stab_core::{
    Circuit, CompiledDetectionConverter, CompiledSampler, DetectionConversionOptions,
    ErrorAnalyzerOptions, Gate, Probability, circuit_flow_generators,
    circuit_to_detector_error_model, sample_detection_events,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::super::{STAB_COMPARE_ITERATIONS, measure_stab_iterations, stab_runner_error};
use super::parse_circuit;

const SPP_EXECUTION_CASES: [&str; 4] = [
    "SPP X0\nM 0\nDETECTOR rec[-1]\n",
    "SPP !X0\nM 0\nDETECTOR rec[-1]\n",
    "SPP X0*X1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
    "SPP_DAG Y0*Y1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
];

const SPP_ANALYZER_CASES: [&str; 4] = [
    "SPP Z0\nS_DAG 0\nM 0\nDETECTOR rec[-1]\n",
    "SPP_DAG Z0\nS 0\nM 0\nDETECTOR rec[-1]\n",
    "SPP !Z0\nS 0\nM 0\nDETECTOR rec[-1]\n",
    "SPP X0\nH 0\nS_DAG 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
];

const EXTENDED_EXECUTION_CASES: [&str; 16] = [
    "X 0\nMR(0.05) !0\nM 0\nDETECTOR rec[-1]\n",
    "R 0 1\nMZZ 0 1\nDETECTOR rec[-1]\n",
    "R 0\nMPP(0.05) Z0\nDETECTOR rec[-1]\n",
    "MPAD(0.05) 0\nDETECTOR rec[-1]\n",
    "R 0\nX_ERROR(0.01) 0\nM 0\nDETECTOR rec[-1]\n",
    "R 0\nPAULI_CHANNEL_1(0.01,0.02,0.03) 0\nM 0\nDETECTOR rec[-1]\n",
    "R 0 1\nPAULI_CHANNEL_2(0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001,0.001) 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
    "R 0 1\nI_ERROR(0.5) 0\nII_ERROR(0.5) 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
    "R 0\nDEPOLARIZE1(0.01) 0\nM 0\nDETECTOR rec[-1]\n",
    "R 0 1\nDEPOLARIZE2(0.01) 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
    "R 0\nE(0.01) X0\nELSE_CORRELATED_ERROR(0.02) Y0\nM 0\nDETECTOR rec[-1]\n",
    "QUBIT_COORDS(1,2) 0\nM 0\nDETECTOR(3) rec[-1]\nSHIFT_COORDS(4)\n",
    "MPAD 1\nCX rec[-1] 0\nM 0\nDETECTOR rec[-1]\n",
    "MPAD 1\nXCZ 0 rec[-1]\nM 0\nDETECTOR rec[-1]\n",
    "MPAD 0 0\nCZ rec[-1] rec[-2]\nM 0\nDETECTOR rec[-1]\n",
    "REPEAT 2 {\n    H 0\n    H 0\n}\nM 0\nDETECTOR rec[-1]\n",
];

const FRAME_TRIGGER_EXECUTION_CASES: [&str; 2] = [
    "HERALDED_ERASE(0.01) 0\nDETECTOR rec[-1]\n",
    "HERALDED_PAULI_CHANNEL_1(0.01,0.01,0.01,0.01) 0\nDETECTOR rec[-1]\n",
];

struct GateSemanticCorpus {
    fixed: Vec<Circuit>,
    spp: Vec<Circuit>,
    spp_analyzer: Vec<Circuit>,
    extended: Vec<Circuit>,
    frame_trigger: Vec<Circuit>,
    forced_frame: Vec<Circuit>,
}

impl GateSemanticCorpus {
    fn new(row_id: &str) -> Result<Self, BenchError> {
        let fixed = fixed_tableau_gate_execution_circuits(row_id)?;
        let spp = parse_cases(row_id, &SPP_EXECUTION_CASES)?;
        let spp_analyzer = parse_cases(row_id, &SPP_ANALYZER_CASES)?;
        let extended = parse_cases(row_id, &EXTENDED_EXECUTION_CASES)?;
        let frame_trigger = parse_cases(row_id, &FRAME_TRIGGER_EXECUTION_CASES)?;
        let forced_frame = fixed
            .iter()
            .chain(&spp)
            .chain(&extended)
            .chain(&frame_trigger)
            .map(|circuit| forced_frame_circuit(row_id, circuit))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            fixed,
            spp,
            spp_analyzer,
            extended,
            frame_trigger,
            forced_frame,
        })
    }

    fn execution_circuits(&self) -> impl Iterator<Item = &Circuit> {
        self.fixed
            .iter()
            .chain(&self.spp)
            .chain(&self.extended)
            .chain(&self.frame_trigger)
    }

    fn ordinary_detection_circuits(&self) -> impl Iterator<Item = &Circuit> {
        self.fixed.iter().chain(&self.spp).chain(&self.extended)
    }

    fn forced_frame_circuits(&self) -> impl Iterator<Item = &Circuit> {
        self.forced_frame.iter()
    }

    fn analyzer_circuits(&self) -> impl Iterator<Item = &Circuit> {
        self.fixed
            .iter()
            .chain(&self.spp_analyzer)
            .chain(&self.extended)
            .chain(&self.frame_trigger)
    }
}

pub(super) fn run(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let corpus = GateSemanticCorpus::new(&row.id)?;
    let samplers = corpus
        .execution_circuits()
        .map(|circuit| {
            CompiledSampler::compile(circuit).map_err(|error| stab_runner_error(&row.id, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let analyzer_options = ErrorAnalyzerOptions {
        approximate_disjoint_errors_threshold: Some(
            Probability::try_new(1.0).map_err(|error| stab_runner_error(&row.id, error))?,
        ),
        ..ErrorAnalyzerOptions::default()
    };

    Ok(vec![
        measure_stab_iterations(
            "stab_pf3_gate_sampler_execution",
            STAB_COMPARE_ITERATIONS,
            || {
                for sampler in &samplers {
                    black_box(sampler.sample_zero_one_with_seed(1, Some(29)));
                }
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_reference_sampling",
            STAB_COMPARE_ITERATIONS,
            || {
                for sampler in &samplers {
                    black_box(sampler.reference_sample());
                }
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_converter_compilation",
            STAB_COMPARE_ITERATIONS,
            || {
                let mut detectors = 0usize;
                for circuit in corpus.execution_circuits() {
                    let converter = CompiledDetectionConverter::compile(
                        circuit,
                        DetectionConversionOptions {
                            skip_reference_sample: false,
                        },
                    )
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                    detectors = detectors
                        .checked_add(converter.detector_count())
                        .ok_or_else(|| {
                            gate_semantic_count_overflow_error(row, "converter detector count")
                        })?;
                }
                black_box(detectors);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_detection_sampling",
            STAB_COMPARE_ITERATIONS,
            || {
                let mut records = 0usize;
                for circuit in corpus.ordinary_detection_circuits() {
                    records = records
                        .checked_add(
                            sample_detection_events(circuit, 1, Some(31))
                                .map_err(|error| stab_runner_error(&row.id, error))?
                                .records
                                .len(),
                        )
                        .ok_or_else(|| {
                            gate_semantic_count_overflow_error(row, "detection record count")
                        })?;
                }
                black_box(records);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_detector_frame_sampling",
            STAB_COMPARE_ITERATIONS,
            || {
                let mut records = 0usize;
                for circuit in corpus.forced_frame_circuits() {
                    records = records
                        .checked_add(
                            sample_detection_events(circuit, 1, Some(31))
                                .map_err(|error| stab_runner_error(&row.id, error))?
                                .records
                                .len(),
                        )
                        .ok_or_else(|| {
                            gate_semantic_count_overflow_error(row, "detector-frame record count")
                        })?;
                }
                black_box(records);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_error_analysis",
            STAB_COMPARE_ITERATIONS,
            || {
                let mut items = 0usize;
                for circuit in corpus.analyzer_circuits() {
                    items = items
                        .checked_add(
                            circuit_to_detector_error_model(circuit, analyzer_options)
                                .map_err(|error| stab_runner_error(&row.id, error))?
                                .items()
                                .len(),
                        )
                        .ok_or_else(|| {
                            gate_semantic_count_overflow_error(row, "analyzer item count")
                        })?;
                }
                black_box(items);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_pf3_gate_flow_generation",
            STAB_COMPARE_ITERATIONS,
            || {
                let mut flows = 0usize;
                for circuit in corpus.execution_circuits() {
                    flows = flows
                        .checked_add(
                            circuit_flow_generators(circuit)
                                .map_err(|error| stab_runner_error(&row.id, error))?
                                .len(),
                        )
                        .ok_or_else(|| gate_semantic_count_overflow_error(row, "flow count"))?;
                }
                black_box(flows);
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(name: &str) -> Option<(f64, &'static str)> {
    let fixed = Gate::all().filter(|gate| gate.has_tableau()).count();
    let execution = fixed
        + SPP_EXECUTION_CASES.len()
        + EXTENDED_EXECUTION_CASES.len()
        + FRAME_TRIGGER_EXECUTION_CASES.len();
    let ordinary_detection = execution - FRAME_TRIGGER_EXECUTION_CASES.len();
    let analyzer = fixed
        + SPP_ANALYZER_CASES.len()
        + EXTENDED_EXECUTION_CASES.len()
        + FRAME_TRIGGER_EXECUTION_CASES.len();
    match name {
        "stab_pf3_gate_sampler_execution"
        | "stab_pf3_gate_reference_sampling"
        | "stab_pf3_gate_converter_compilation"
        | "stab_pf3_gate_detector_frame_sampling"
        | "stab_pf3_gate_flow_generation" => Some((execution as f64, "circuits/s")),
        "stab_pf3_gate_detection_sampling" => Some((ordinary_detection as f64, "circuits/s")),
        "stab_pf3_gate_error_analysis" => Some((analyzer as f64, "circuits/s")),
        _ => None,
    }
}

fn forced_frame_circuit(row_id: &str, circuit: &Circuit) -> Result<Circuit, BenchError> {
    parse_circuit(
        row_id,
        &format!("{}OBSERVABLE_INCLUDE(0) Z0\n", circuit.to_stim_string()),
    )
}

fn parse_cases<const N: usize>(
    row_id: &str,
    cases: &[&str; N],
) -> Result<Vec<Circuit>, BenchError> {
    cases
        .iter()
        .map(|text| parse_circuit(row_id, text))
        .collect()
}

fn fixed_tableau_gate_execution_circuits(row_id: &str) -> Result<Vec<Circuit>, BenchError> {
    Gate::all()
        .filter(|gate| gate.has_tableau())
        .map(|gate| fixed_tableau_gate_execution_circuit(row_id, gate))
        .collect()
}

fn fixed_tableau_gate_execution_circuit(row_id: &str, gate: Gate) -> Result<Circuit, BenchError> {
    let gate_name = gate.canonical_name();
    let inverse_name = gate
        .inverse()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("{gate_name} has tableau metadata but no inverse"),
        })?
        .canonical_name();
    let arity = gate
        .tableau()
        .map_err(|error| stab_runner_error(row_id, error))?
        .len();
    let targets = match arity {
        1 => "0",
        2 => "0 1",
        _ => {
            return Err(BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: format!("{gate_name} has unsupported benchmark arity {arity}"),
            });
        }
    };
    parse_circuit(
        row_id,
        &format!("{gate_name} {targets}\n{inverse_name} {targets}\nM 0\nDETECTOR rec[-1]\n"),
    )
}

fn gate_semantic_count_overflow_error(row: &BenchmarkRow, context: &str) -> BenchError {
    BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!("PF3 gate semantic benchmark overflowed while counting {context}"),
    }
}
