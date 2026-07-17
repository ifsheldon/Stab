use super::{
    BitAcceptanceContract, Implementation, InputDigest, InvocationError, PreparedWorkers,
    ProcessResult, SemanticDigest, WorkerContractProbeEvidence, rejected_probe,
};

pub(in crate::qualification::runtime) const PAULI_ITER_INPUT_BYTES: u64 = 64;
pub(super) const PAULI_ITER_RANGE_SMALL_WORK_ITEMS: u64 = 232;
pub(super) const PAULI_ITER_RANGE_MAX_WORK_ITEMS: u64 = 972_972;
pub(super) const PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS: u64 = 3_000;
pub(super) const PAULI_ITER_SINGLETON_MAX_WORK_ITEMS: u64 = 3_145_728;

pub(super) const PAULI_ITER_RANGE_SMALL_INPUT_DIGEST: &str =
    "315732711c88257f9f4b2be3dfc3dd01785be01b86bdb7338e663945a90070d5";
pub(super) const PAULI_ITER_RANGE_MAX_INPUT_DIGEST: &str =
    "85017fcee6d99c399676aac24ff1945f03363f316352eb10d707b51c66f506bc";
pub(super) const PAULI_ITER_SINGLETON_SMALL_INPUT_DIGEST: &str =
    "d8d6b42d21392b7ab593f2b09cb9673e261381aa2d93c8f15b8c4ac52a97235b";
pub(super) const PAULI_ITER_SINGLETON_MAX_INPUT_DIGEST: &str =
    "7030fe57e3a362ae0fb7339fe57022d17117c00109d5770a9cd1a17ef2aeb004";

pub(super) const PAULI_ITER_RANGE_ODD_OUTPUT_DIGEST: &str =
    "33ec6ca5492ec7c41dd77336a39ac12a6d360e2908296e185bf2bc595eea8a3b";
pub(super) const PAULI_ITER_RANGE_EVEN_OUTPUT_DIGEST: &str =
    "2f6bc04a3bc5dbe5e31ecc23e35dbb43654a86e450d9a504384977067ea5e236";
pub(super) const PAULI_ITER_RANGE_MAX_OUTPUT_DIGEST: &str =
    "9cb202f2fe7298a94e9b70c6cc6013fcec84fbdc2882d3002ba47be67a9a27ef";
pub(super) const PAULI_ITER_SINGLETON_ODD_OUTPUT_DIGEST: &str =
    "636eefbc6bb18c4b610b2a7841fa225d900cf8c0e1d140f19ef135c71d0ee8b0";
pub(super) const PAULI_ITER_SINGLETON_EVEN_OUTPUT_DIGEST: &str =
    "f91eddf2188a63ea8d7461f1b057911088b819852bdbc3250b64e7dbb39dfdb5";
pub(super) const PAULI_ITER_SINGLETON_MAX_OUTPUT_DIGEST: &str =
    "dc139407ec96818d8e3ee508abca24958230f0955b313d6b287ccafcb5ba2872";

pub(super) const PAULI_ITER_RANGE_ODD_CASE_ID: &str = "pauli-string-iter-range-small-odd";
pub(super) const PAULI_ITER_RANGE_EVEN_CASE_ID: &str = "pauli-string-iter-range-small-even";
pub(super) const PAULI_ITER_RANGE_MAX_CASE_ID: &str = "pauli-string-iter-range-maximum";
pub(super) const PAULI_ITER_SINGLETON_ODD_CASE_ID: &str = "pauli-string-iter-singleton-small-odd";
pub(super) const PAULI_ITER_SINGLETON_EVEN_CASE_ID: &str = "pauli-string-iter-singleton-small-even";
pub(super) const PAULI_ITER_SINGLETON_MAX_CASE_ID: &str = "pauli-string-iter-singleton-maximum";

const RANGE_MALFORMED_WORK_ITEMS: &str = "233";
const RANGE_OVER_CAP_WORK_ITEMS: &str = "1233628";
const RANGE_SMALL_WORK_ITEMS_TEXT: &str = "232";
const RANGE_WORK_OVERFLOW_ITERATIONS: &str = "79511827903920482";
const RANGE_WIDTH_OVERFLOW_ITERATIONS: &str = "15902365580784097";
const SINGLETON_MALFORMED_WORK_ITEMS: &str = "3001";
const SINGLETON_OVER_CAP_WORK_ITEMS: &str = "3145731";
const SINGLETON_SMALL_WORK_ITEMS_TEXT: &str = "3000";
const SINGLETON_WORK_OVERFLOW_ITERATIONS: &str = "6148914691236518";
const SINGLETON_WIDTH_OVERFLOW_ITERATIONS: &str = "6148914691237";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification::runtime) enum PauliIterContractKind {
    Range,
    Singleton,
}

impl PauliIterContractKind {
    pub(in crate::qualification::runtime) const fn all() -> [Self; 2] {
        [Self::Range, Self::Singleton]
    }

    pub(in crate::qualification::runtime) const fn workload(self) -> &'static str {
        match self {
            Self::Range => "pauli-string-iter-range",
            Self::Singleton => "pauli-string-iter-singleton",
        }
    }

    pub(in crate::qualification::runtime) const fn measurement(self) -> &'static str {
        "construct-and-iterate-borrowed"
    }

    pub(in crate::qualification::runtime) const fn maximum_work_items(self) -> u64 {
        match self {
            Self::Range => PAULI_ITER_RANGE_MAX_WORK_ITEMS,
            Self::Singleton => PAULI_ITER_SINGLETON_MAX_WORK_ITEMS,
        }
    }

    pub(in crate::qualification::runtime) const fn maximum_input_digest(self) -> &'static str {
        match self {
            Self::Range => PAULI_ITER_RANGE_MAX_INPUT_DIGEST,
            Self::Singleton => PAULI_ITER_SINGLETON_MAX_INPUT_DIGEST,
        }
    }

    pub(in crate::qualification::runtime) const fn maximum_output_digest(self) -> &'static str {
        match self {
            Self::Range => PAULI_ITER_RANGE_MAX_OUTPUT_DIGEST,
            Self::Singleton => PAULI_ITER_SINGLETON_MAX_OUTPUT_DIGEST,
        }
    }

    const fn small_work_items(self) -> &'static str {
        match self {
            Self::Range => RANGE_SMALL_WORK_ITEMS_TEXT,
            Self::Singleton => SINGLETON_SMALL_WORK_ITEMS_TEXT,
        }
    }

    const fn malformed_work_items(self) -> &'static str {
        match self {
            Self::Range => RANGE_MALFORMED_WORK_ITEMS,
            Self::Singleton => SINGLETON_MALFORMED_WORK_ITEMS,
        }
    }

    const fn over_cap_work_items(self) -> &'static str {
        match self {
            Self::Range => RANGE_OVER_CAP_WORK_ITEMS,
            Self::Singleton => SINGLETON_OVER_CAP_WORK_ITEMS,
        }
    }

    const fn work_overflow_iterations(self) -> &'static str {
        match self {
            Self::Range => RANGE_WORK_OVERFLOW_ITERATIONS,
            Self::Singleton => SINGLETON_WORK_OVERFLOW_ITERATIONS,
        }
    }

    const fn width_overflow_iterations(self) -> &'static str {
        match self {
            Self::Range => RANGE_WIDTH_OVERFLOW_ITERATIONS,
            Self::Singleton => SINGLETON_WIDTH_OVERFLOW_ITERATIONS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification::runtime) enum PauliIterRejectionClass {
    Zero,
    MalformedShape,
    OverCap,
    WrongMeasurement,
    WorkOverflow,
    WidthChecksumOverflow,
}

impl PauliIterRejectionClass {
    pub(in crate::qualification::runtime) const fn all() -> [Self; 6] {
        [
            Self::Zero,
            Self::MalformedShape,
            Self::OverCap,
            Self::WrongMeasurement,
            Self::WorkOverflow,
            Self::WidthChecksumOverflow,
        ]
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::MalformedShape => "malformed-shape",
            Self::OverCap => "over-cap",
            Self::WrongMeasurement => "wrong-measurement",
            Self::WorkOverflow => "work-overflow",
            Self::WidthChecksumOverflow => "width-checksum-overflow",
        }
    }

    pub(super) fn case_id(self, kind: PauliIterContractKind) -> &'static str {
        match (kind, self) {
            (PauliIterContractKind::Range, Self::Zero) => "pauli-string-iter-range-zero",
            (PauliIterContractKind::Range, Self::MalformedShape) => {
                "pauli-string-iter-range-malformed-shape"
            }
            (PauliIterContractKind::Range, Self::OverCap) => "pauli-string-iter-range-over-cap",
            (PauliIterContractKind::Range, Self::WrongMeasurement) => {
                "pauli-string-iter-range-wrong-measurement"
            }
            (PauliIterContractKind::Range, Self::WorkOverflow) => {
                "pauli-string-iter-range-work-overflow"
            }
            (PauliIterContractKind::Range, Self::WidthChecksumOverflow) => {
                "pauli-string-iter-range-width-checksum-overflow"
            }
            (PauliIterContractKind::Singleton, Self::Zero) => "pauli-string-iter-singleton-zero",
            (PauliIterContractKind::Singleton, Self::MalformedShape) => {
                "pauli-string-iter-singleton-malformed-shape"
            }
            (PauliIterContractKind::Singleton, Self::OverCap) => {
                "pauli-string-iter-singleton-over-cap"
            }
            (PauliIterContractKind::Singleton, Self::WrongMeasurement) => {
                "pauli-string-iter-singleton-wrong-measurement"
            }
            (PauliIterContractKind::Singleton, Self::WorkOverflow) => {
                "pauli-string-iter-singleton-work-overflow"
            }
            (PauliIterContractKind::Singleton, Self::WidthChecksumOverflow) => {
                "pauli-string-iter-singleton-width-checksum-overflow"
            }
        }
    }

    pub(in crate::qualification::runtime) const fn measurement(self) -> &'static str {
        match self {
            Self::WrongMeasurement => "wrong",
            _ => "construct-and-iterate-borrowed",
        }
    }

    pub(in crate::qualification::runtime) const fn iterations(
        self,
        kind: PauliIterContractKind,
    ) -> &'static str {
        match self {
            Self::WorkOverflow => kind.work_overflow_iterations(),
            Self::WidthChecksumOverflow => kind.width_overflow_iterations(),
            _ => "1",
        }
    }

    pub(in crate::qualification::runtime) const fn work_items(
        self,
        kind: PauliIterContractKind,
    ) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::MalformedShape => kind.malformed_work_items(),
            Self::OverCap => kind.over_cap_work_items(),
            Self::WrongMeasurement | Self::WorkOverflow | Self::WidthChecksumOverflow => {
                kind.small_work_items()
            }
        }
    }
}

impl PreparedWorkers {
    pub(super) fn invoke_pauli_iter_acceptance_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let range_small_input = InputDigest::try_new(PAULI_ITER_RANGE_SMALL_INPUT_DIGEST)?;
        let range_max_input = InputDigest::try_new(PAULI_ITER_RANGE_MAX_INPUT_DIGEST)?;
        let singleton_small_input = InputDigest::try_new(PAULI_ITER_SINGLETON_SMALL_INPUT_DIGEST)?;
        let singleton_max_input = InputDigest::try_new(PAULI_ITER_SINGLETON_MAX_INPUT_DIGEST)?;
        let range_odd = SemanticDigest::try_new(PAULI_ITER_RANGE_ODD_OUTPUT_DIGEST)?;
        let range_even = SemanticDigest::try_new(PAULI_ITER_RANGE_EVEN_OUTPUT_DIGEST)?;
        let range_max = SemanticDigest::try_new(PAULI_ITER_RANGE_MAX_OUTPUT_DIGEST)?;
        let singleton_odd = SemanticDigest::try_new(PAULI_ITER_SINGLETON_ODD_OUTPUT_DIGEST)?;
        let singleton_even = SemanticDigest::try_new(PAULI_ITER_SINGLETON_EVEN_OUTPUT_DIGEST)?;
        let singleton_max = SemanticDigest::try_new(PAULI_ITER_SINGLETON_MAX_OUTPUT_DIGEST)?;
        let mut probes = Vec::with_capacity(12);
        for implementation in [Implementation::Stim, Implementation::Stab] {
            for contract in [
                BitAcceptanceContract {
                    case_id: PAULI_ITER_RANGE_ODD_CASE_ID,
                    workload: PauliIterContractKind::Range.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 1,
                    work_items: PAULI_ITER_RANGE_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &range_small_input,
                    expected_output_digest: &range_odd,
                },
                BitAcceptanceContract {
                    case_id: PAULI_ITER_RANGE_EVEN_CASE_ID,
                    workload: PauliIterContractKind::Range.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 2,
                    work_items: PAULI_ITER_RANGE_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &range_small_input,
                    expected_output_digest: &range_even,
                },
                BitAcceptanceContract {
                    case_id: PAULI_ITER_RANGE_MAX_CASE_ID,
                    workload: PauliIterContractKind::Range.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 1,
                    work_items: PAULI_ITER_RANGE_MAX_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &range_max_input,
                    expected_output_digest: &range_max,
                },
                BitAcceptanceContract {
                    case_id: PAULI_ITER_SINGLETON_ODD_CASE_ID,
                    workload: PauliIterContractKind::Singleton.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 1,
                    work_items: PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &singleton_small_input,
                    expected_output_digest: &singleton_odd,
                },
                BitAcceptanceContract {
                    case_id: PAULI_ITER_SINGLETON_EVEN_CASE_ID,
                    workload: PauliIterContractKind::Singleton.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 2,
                    work_items: PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &singleton_small_input,
                    expected_output_digest: &singleton_even,
                },
                BitAcceptanceContract {
                    case_id: PAULI_ITER_SINGLETON_MAX_CASE_ID,
                    workload: PauliIterContractKind::Singleton.workload(),
                    measurement: "construct-and-iterate-borrowed",
                    iterations: 1,
                    work_items: PAULI_ITER_SINGLETON_MAX_WORK_ITEMS,
                    input_bytes: PAULI_ITER_INPUT_BYTES,
                    expected_input_digest: &singleton_max_input,
                    expected_output_digest: &singleton_max,
                },
            ] {
                probes.push(self.invoke_bit_acceptance(implementation, contract)?);
            }
        }
        Ok(probes)
    }

    pub(super) fn invoke_pauli_iter_rejection_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let mut probes = Vec::with_capacity(24);
        for kind in PauliIterContractKind::all() {
            for class in PauliIterRejectionClass::all() {
                for implementation in [Implementation::Stim, Implementation::Stab] {
                    let output = self.invoke_invalid_work(
                        implementation,
                        kind.workload(),
                        class.measurement(),
                        class.iterations(kind),
                        class.work_items(kind),
                    )?;
                    checked_pauli_iter_rejection(&output, implementation, kind, class)?;
                    probes.push(rejected_probe(
                        class.case_id(kind),
                        implementation,
                        &output,
                    )?);
                }
            }
        }
        Ok(probes)
    }
}

pub(in crate::qualification::runtime) fn checked_pauli_iter_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    kind: PauliIterContractKind,
    class: PauliIterRejectionClass,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        pauli_iter_rejection_expectation(implementation, kind, class);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::PauliIterWorkRejection {
            implementation,
            workload: kind.workload(),
            class: class.label(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn pauli_iter_rejection_expectation(
    implementation: Implementation,
    kind: PauliIterContractKind,
    class: PauliIterRejectionClass,
) -> (i32, &'static str) {
    match (implementation, kind, class) {
        (Implementation::Stim, _, PauliIterRejectionClass::Zero) => (
            2,
            "stim qualification adapter: work-items must be positive\n",
        ),
        (Implementation::Stab, _, PauliIterRejectionClass::Zero) => (
            2,
            "error: invalid value '0' for '--work-items <WORK_ITEMS>': number would be zero for non-zero type\n\nFor more information, try '--help'.\n",
        ),
        (
            Implementation::Stim,
            PauliIterContractKind::Range,
            PauliIterRejectionClass::MalformedShape,
        ) => (
            2,
            "stim qualification adapter: pauli-string-iter-range work count 233 is not a complete source-owned iterator traversal\n",
        ),
        (
            Implementation::Stab,
            PauliIterContractKind::Range,
            PauliIterRejectionClass::MalformedShape,
        ) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\npauli-string-iter-range work count 233 is not a complete source-owned iterator traversal\n",
        ),
        (
            Implementation::Stim,
            PauliIterContractKind::Singleton,
            PauliIterRejectionClass::MalformedShape,
        ) => (
            2,
            "stim qualification adapter: pauli-string-iter-singleton work count 3001 is not a complete source-owned iterator traversal\n",
        ),
        (
            Implementation::Stab,
            PauliIterContractKind::Singleton,
            PauliIterRejectionClass::MalformedShape,
        ) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\npauli-string-iter-singleton work count 3001 is not a complete source-owned iterator traversal\n",
        ),
        (Implementation::Stim, PauliIterContractKind::Range, PauliIterRejectionClass::OverCap) => (
            2,
            "stim qualification adapter: pauli-string-iter-range output count 1233628 exceeds maximum 1000000\n",
        ),
        (Implementation::Stab, PauliIterContractKind::Range, PauliIterRejectionClass::OverCap) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\npauli-string-iter-range output count 1233628 exceeds maximum 1000000\n",
        ),
        (
            Implementation::Stim,
            PauliIterContractKind::Singleton,
            PauliIterRejectionClass::OverCap,
        ) => (
            2,
            "stim qualification adapter: pauli-string-iter-singleton width 1048577 exceeds maximum 1048576\n",
        ),
        (
            Implementation::Stab,
            PauliIterContractKind::Singleton,
            PauliIterRejectionClass::OverCap,
        ) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\npauli-string-iter-singleton width 1048577 exceeds maximum 1048576\n",
        ),
        (Implementation::Stim, _, PauliIterRejectionClass::WrongMeasurement) => (
            2,
            "stim qualification adapter: adapter workload and measurement are not a registered pair\n",
        ),
        (
            Implementation::Stab,
            PauliIterContractKind::Range,
            PauliIterRejectionClass::WrongMeasurement,
        ) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nqualification workload pauli-string-iter-range requires measurement construct-and-iterate-borrowed, got wrong\n",
        ),
        (
            Implementation::Stab,
            PauliIterContractKind::Singleton,
            PauliIterRejectionClass::WrongMeasurement,
        ) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nqualification workload pauli-string-iter-singleton requires measurement construct-and-iterate-borrowed, got wrong\n",
        ),
        (Implementation::Stim, _, PauliIterRejectionClass::WorkOverflow) => (
            2,
            "stim qualification adapter: adapter semantic work count overflows u64\n",
        ),
        (Implementation::Stab, _, PauliIterRejectionClass::WorkOverflow) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nqualification worker semantic work count overflows u64\n",
        ),
        (Implementation::Stim, _, PauliIterRejectionClass::WidthChecksumOverflow) => (
            2,
            "stim qualification adapter: Pauli iterator output-count times result-width checksum overflows u64\n",
        ),
        (Implementation::Stab, _, PauliIterRejectionClass::WidthChecksumOverflow) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nPauli iterator output-count times result-width checksum overflows u64\n",
        ),
    }
}
