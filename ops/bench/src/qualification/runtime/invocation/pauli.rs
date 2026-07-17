use super::{
    BitAcceptanceContract, Implementation, InputDigest, InvocationError, PreparedWorkers,
    ProcessResult, SemanticDigest, WorkerContractProbeEvidence, rejected_probe,
};

pub(super) const PAULI_SMALL_WORK_ITEMS: u64 = 10_000;
pub(super) const PAULI_MAX_WORK_ITEMS: u64 = 1_048_576;
pub(super) const PAULI_SMALL_INPUT_BYTES: u64 = 5_056;
pub(super) const PAULI_MAX_INPUT_BYTES: u64 = 524_320;
pub(super) const PAULI_SMALL_INPUT_DIGEST: &str =
    "401b897ceb9c02fec1c57b15f76cdc45045fd551354c3dc5ae499e791aef22f4";
pub(super) const PAULI_MAX_INPUT_DIGEST: &str =
    "404403b9507220987eff4ee0fea6d6794029fd9bbda3c8b3ea5b4379cfb2d009";
pub(super) const PAULI_ODD_OUTPUT_DIGEST: &str =
    "295e7945d9961ad35f77e614b5c3c9ae84f419db221f53f0e609eb77fe773269";
pub(super) const PAULI_EVEN_OUTPUT_DIGEST: &str =
    "89e436e86731c707ad1baa48ca83f1d69d21fe61aed075ca60485642d0c4b0bd";
pub(super) const PAULI_MAX_OUTPUT_DIGEST: &str =
    "b3fce0417dc4a2c5c91c2d79fe36f7d67759056ad86261ed20f1a9ba4e9e1848";

pub(super) const PAULI_ODD_CASE_ID: &str = "pauli-string-right-multiply-small-odd";
pub(super) const PAULI_EVEN_CASE_ID: &str = "pauli-string-right-multiply-small-even";
pub(super) const PAULI_MAX_CASE_ID: &str = "pauli-string-right-multiply-maximum";

const PAULI_ZERO_WORK_ITEMS: &str = "0";
const PAULI_OVER_CAP_WORK_ITEMS: &str = "1048577";
const PAULI_MAX_WORK_ITEMS_TEXT: &str = "1048576";
const PAULI_OVERFLOW_ITERATIONS: &str = "17592186044416";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PauliRejectionClass {
    Zero,
    OverCap,
    WrongMeasurement,
    WorkOverflow,
}

impl PauliRejectionClass {
    pub(super) const fn all() -> [Self; 4] {
        [
            Self::Zero,
            Self::OverCap,
            Self::WrongMeasurement,
            Self::WorkOverflow,
        ]
    }

    pub(super) const fn case_id(self) -> &'static str {
        match self {
            Self::Zero => "pauli-string-right-multiply-zero",
            Self::OverCap => "pauli-string-right-multiply-over-cap",
            Self::WrongMeasurement => "pauli-string-right-multiply-wrong-measurement",
            Self::WorkOverflow => "pauli-string-right-multiply-work-overflow",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::OverCap => "over-cap",
            Self::WrongMeasurement => "wrong-measurement",
            Self::WorkOverflow => "work-overflow",
        }
    }

    const fn measurement(self) -> &'static str {
        match self {
            Self::WrongMeasurement => "wrong",
            _ => "right-multiply-in-place",
        }
    }

    const fn iterations(self) -> &'static str {
        match self {
            Self::WorkOverflow => PAULI_OVERFLOW_ITERATIONS,
            _ => "1",
        }
    }

    const fn work_items(self) -> &'static str {
        match self {
            Self::Zero => PAULI_ZERO_WORK_ITEMS,
            Self::OverCap => PAULI_OVER_CAP_WORK_ITEMS,
            Self::WrongMeasurement => "1",
            Self::WorkOverflow => PAULI_MAX_WORK_ITEMS_TEXT,
        }
    }
}

impl PreparedWorkers {
    pub(super) fn invoke_pauli_acceptance_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let small_input = InputDigest::try_new(PAULI_SMALL_INPUT_DIGEST)?;
        let max_input = InputDigest::try_new(PAULI_MAX_INPUT_DIGEST)?;
        let odd_output = SemanticDigest::try_new(PAULI_ODD_OUTPUT_DIGEST)?;
        let even_output = SemanticDigest::try_new(PAULI_EVEN_OUTPUT_DIGEST)?;
        let max_output = SemanticDigest::try_new(PAULI_MAX_OUTPUT_DIGEST)?;
        let mut probes = Vec::with_capacity(6);
        for implementation in [Implementation::Stim, Implementation::Stab] {
            for contract in [
                BitAcceptanceContract {
                    case_id: PAULI_ODD_CASE_ID,
                    workload: "pauli-string-right-multiply",
                    measurement: "right-multiply-in-place",
                    iterations: 1,
                    work_items: PAULI_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &odd_output,
                },
                BitAcceptanceContract {
                    case_id: PAULI_EVEN_CASE_ID,
                    workload: "pauli-string-right-multiply",
                    measurement: "right-multiply-in-place",
                    iterations: 2,
                    work_items: PAULI_SMALL_WORK_ITEMS,
                    input_bytes: PAULI_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &even_output,
                },
                BitAcceptanceContract {
                    case_id: PAULI_MAX_CASE_ID,
                    workload: "pauli-string-right-multiply",
                    measurement: "right-multiply-in-place",
                    iterations: 1,
                    work_items: PAULI_MAX_WORK_ITEMS,
                    input_bytes: PAULI_MAX_INPUT_BYTES,
                    expected_input_digest: &max_input,
                    expected_output_digest: &max_output,
                },
            ] {
                probes.push(self.invoke_bit_acceptance(implementation, contract)?);
            }
        }
        Ok(probes)
    }

    pub(super) fn invoke_pauli_rejection_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let mut probes = Vec::with_capacity(8);
        for class in PauliRejectionClass::all() {
            for implementation in [Implementation::Stim, Implementation::Stab] {
                let output = self.invoke_invalid_work(
                    implementation,
                    "pauli-string-right-multiply",
                    class.measurement(),
                    class.iterations(),
                    class.work_items(),
                )?;
                checked_pauli_rejection(&output, implementation, class)?;
                probes.push(rejected_probe(class.case_id(), implementation, &output)?);
            }
        }
        Ok(probes)
    }
}

pub(super) fn checked_pauli_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    class: PauliRejectionClass,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = pauli_rejection_expectation(implementation, class);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::PauliWorkRejection {
            implementation,
            class: class.label(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn pauli_rejection_expectation(
    implementation: Implementation,
    class: PauliRejectionClass,
) -> (i32, &'static str) {
    match (implementation, class) {
        (Implementation::Stim, PauliRejectionClass::Zero) => (
            2,
            "stim qualification adapter: work-items must be positive\n",
        ),
        (Implementation::Stab, PauliRejectionClass::Zero) => (
            2,
            "error: invalid value '0' for '--work-items <WORK_ITEMS>': number would be zero for non-zero type\n\nFor more information, try '--help'.\n",
        ),
        (Implementation::Stim, PauliRejectionClass::OverCap) => (
            2,
            "stim qualification adapter: Pauli multiplication width 1048577 exceeds maximum 1048576\n",
        ),
        (Implementation::Stab, PauliRejectionClass::OverCap) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nPauli multiplication width 1048577 exceeds maximum 1048576\n",
        ),
        (Implementation::Stim, PauliRejectionClass::WrongMeasurement) => (
            2,
            "stim qualification adapter: adapter workload and measurement are not a registered pair\n",
        ),
        (Implementation::Stab, PauliRejectionClass::WrongMeasurement) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nqualification workload pauli-string-right-multiply requires measurement right-multiply-in-place, got wrong\n",
        ),
        (Implementation::Stim, PauliRejectionClass::WorkOverflow) => (
            2,
            "stim qualification adapter: adapter semantic work count overflows u64\n",
        ),
        (Implementation::Stab, PauliRejectionClass::WorkOverflow) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nqualification worker semantic work count overflows u64\n",
        ),
    }
}
