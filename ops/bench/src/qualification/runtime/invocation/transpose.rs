use super::{
    BitAcceptanceContract, Implementation, InputDigest, InvocationError, PreparedWorkers,
    ProcessResult, SemanticDigest, WorkerContractProbeEvidence, rejected_probe,
};

pub(super) const TRANSPOSE_SMALL_WORK_ITEMS: u64 = 65_536;
pub(super) const TRANSPOSE_MAX_WORK_ITEMS: u64 = 268_435_456;
pub(super) const TRANSPOSE_SMALL_INPUT_BYTES: u64 = 8_208;
pub(super) const TRANSPOSE_MAX_INPUT_BYTES: u64 = 33_554_448;
pub(super) const TRANSPOSE_SMALL_INPUT_DIGEST: &str =
    "2a2a5f587d3c9fdb6fea43274c06ad453fcc76bbbcf6bcd9563991076cdf79da";
pub(super) const TRANSPOSE_MAX_INPUT_DIGEST: &str =
    "d68c253c0ca01452ce0624f0fdeb67dd92c85b442034b4b0e574286f3c9f636e";
pub(super) const TRANSPOSE_IN_PLACE_ODD_OUTPUT_DIGEST: &str =
    "ff51fae4355733df7b3982f6daa235aba38d942553ee115340cc736c728421df";
pub(super) const TRANSPOSE_IN_PLACE_EVEN_OUTPUT_DIGEST: &str =
    "de2f8204bcc441d6b20f738e6574c3f5020f2ea07adaf9c23e0a59d183477a23";
pub(super) const TRANSPOSE_IN_PLACE_MAX_OUTPUT_DIGEST: &str =
    "d81fc0d732588e992e3f99542618f8cfa6affb401d5505b0c74efaab8c7f156a";
pub(super) const TRANSPOSE_ALLOCATING_ODD_OUTPUT_DIGEST: &str =
    "47f71e7254cf47c483f4574713cb1c8bee018181e19c218f7c19a4474a8c6373";
pub(super) const TRANSPOSE_ALLOCATING_EVEN_OUTPUT_DIGEST: &str =
    "6f0c4bdf0e761601a1545f63299a06bc37a7e59dcff098ac8ddc5619b9511641";
pub(super) const TRANSPOSE_ALLOCATING_MAX_OUTPUT_DIGEST: &str =
    "4b0e6174ee44ad29107bbe4e60df501c8d64c16d7e464e4d85063f2732391133";

pub(super) const TRANSPOSE_IN_PLACE_ODD_CASE_ID: &str = "bit-matrix-transpose-in-place-small-odd";
pub(super) const TRANSPOSE_IN_PLACE_EVEN_CASE_ID: &str = "bit-matrix-transpose-in-place-small-even";
pub(super) const TRANSPOSE_IN_PLACE_MAX_CASE_ID: &str = "bit-matrix-transpose-in-place-maximum";
pub(super) const TRANSPOSE_ALLOCATING_ODD_CASE_ID: &str =
    "bit-matrix-transpose-allocating-small-odd";
pub(super) const TRANSPOSE_ALLOCATING_EVEN_CASE_ID: &str =
    "bit-matrix-transpose-allocating-small-even";
pub(super) const TRANSPOSE_ALLOCATING_MAX_CASE_ID: &str = "bit-matrix-transpose-allocating-maximum";

const TRANSPOSE_BELOW_MINIMUM_WORK_ITEMS: &str = "65025";
const TRANSPOSE_NON_SQUARE_WORK_ITEMS: &str = "65537";
const TRANSPOSE_UNALIGNED_WORK_ITEMS: &str = "66049";
const TRANSPOSE_OVER_CAP_WORK_ITEMS: &str = "276889600";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransposeRejectionClass {
    InPlaceBelowMinimum,
    InPlaceNonSquare,
    InPlaceUnaligned,
    InPlaceOverCap,
    AllocatingBelowMinimum,
    AllocatingNonSquare,
    AllocatingUnaligned,
    AllocatingOverCap,
}

impl TransposeRejectionClass {
    pub(super) const fn all() -> [Self; 8] {
        [
            Self::InPlaceBelowMinimum,
            Self::InPlaceNonSquare,
            Self::InPlaceUnaligned,
            Self::InPlaceOverCap,
            Self::AllocatingBelowMinimum,
            Self::AllocatingNonSquare,
            Self::AllocatingUnaligned,
            Self::AllocatingOverCap,
        ]
    }

    pub(super) const fn case_id(self) -> &'static str {
        match self {
            Self::InPlaceBelowMinimum => "bit-matrix-transpose-in-place-below-minimum",
            Self::InPlaceNonSquare => "bit-matrix-transpose-in-place-non-square",
            Self::InPlaceUnaligned => "bit-matrix-transpose-in-place-unaligned",
            Self::InPlaceOverCap => "bit-matrix-transpose-in-place-over-cap",
            Self::AllocatingBelowMinimum => "bit-matrix-transpose-allocating-below-minimum",
            Self::AllocatingNonSquare => "bit-matrix-transpose-allocating-non-square",
            Self::AllocatingUnaligned => "bit-matrix-transpose-allocating-unaligned",
            Self::AllocatingOverCap => "bit-matrix-transpose-allocating-over-cap",
        }
    }

    const fn workload(self) -> &'static str {
        match self {
            Self::InPlaceBelowMinimum
            | Self::InPlaceNonSquare
            | Self::InPlaceUnaligned
            | Self::InPlaceOverCap => "bit-matrix-transpose-in-place",
            Self::AllocatingBelowMinimum
            | Self::AllocatingNonSquare
            | Self::AllocatingUnaligned
            | Self::AllocatingOverCap => "bit-matrix-transpose-allocating",
        }
    }

    const fn measurement(self) -> &'static str {
        match self {
            Self::InPlaceBelowMinimum
            | Self::InPlaceNonSquare
            | Self::InPlaceUnaligned
            | Self::InPlaceOverCap => "in-place-transpose",
            Self::AllocatingBelowMinimum
            | Self::AllocatingNonSquare
            | Self::AllocatingUnaligned
            | Self::AllocatingOverCap => "allocating-transpose",
        }
    }

    const fn work_items(self) -> &'static str {
        match self {
            Self::InPlaceBelowMinimum | Self::AllocatingBelowMinimum => {
                TRANSPOSE_BELOW_MINIMUM_WORK_ITEMS
            }
            Self::InPlaceNonSquare | Self::AllocatingNonSquare => TRANSPOSE_NON_SQUARE_WORK_ITEMS,
            Self::InPlaceUnaligned | Self::AllocatingUnaligned => TRANSPOSE_UNALIGNED_WORK_ITEMS,
            Self::InPlaceOverCap | Self::AllocatingOverCap => TRANSPOSE_OVER_CAP_WORK_ITEMS,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::InPlaceBelowMinimum => "in-place-below-minimum",
            Self::InPlaceNonSquare => "in-place-non-square",
            Self::InPlaceUnaligned => "in-place-unaligned",
            Self::InPlaceOverCap => "in-place-over-cap",
            Self::AllocatingBelowMinimum => "allocating-below-minimum",
            Self::AllocatingNonSquare => "allocating-non-square",
            Self::AllocatingUnaligned => "allocating-unaligned",
            Self::AllocatingOverCap => "allocating-over-cap",
        }
    }

    const fn failure(self) -> TransposeShapeFailure {
        match self {
            Self::InPlaceBelowMinimum | Self::AllocatingBelowMinimum => {
                TransposeShapeFailure::BelowMinimum
            }
            Self::InPlaceNonSquare | Self::AllocatingNonSquare => TransposeShapeFailure::NonSquare,
            Self::InPlaceUnaligned | Self::AllocatingUnaligned => TransposeShapeFailure::Unaligned,
            Self::InPlaceOverCap | Self::AllocatingOverCap => TransposeShapeFailure::OverCap,
        }
    }
}

#[derive(Clone, Copy)]
enum TransposeShapeFailure {
    BelowMinimum,
    NonSquare,
    Unaligned,
    OverCap,
}

impl PreparedWorkers {
    pub(super) fn invoke_transpose_acceptance_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let small_input = InputDigest::try_new(TRANSPOSE_SMALL_INPUT_DIGEST)?;
        let max_input = InputDigest::try_new(TRANSPOSE_MAX_INPUT_DIGEST)?;
        let in_place_odd = SemanticDigest::try_new(TRANSPOSE_IN_PLACE_ODD_OUTPUT_DIGEST)?;
        let in_place_even = SemanticDigest::try_new(TRANSPOSE_IN_PLACE_EVEN_OUTPUT_DIGEST)?;
        let in_place_max = SemanticDigest::try_new(TRANSPOSE_IN_PLACE_MAX_OUTPUT_DIGEST)?;
        let allocating_odd = SemanticDigest::try_new(TRANSPOSE_ALLOCATING_ODD_OUTPUT_DIGEST)?;
        let allocating_even = SemanticDigest::try_new(TRANSPOSE_ALLOCATING_EVEN_OUTPUT_DIGEST)?;
        let allocating_max = SemanticDigest::try_new(TRANSPOSE_ALLOCATING_MAX_OUTPUT_DIGEST)?;
        let mut probes = Vec::with_capacity(12);
        for implementation in [Implementation::Stim, Implementation::Stab] {
            for contract in [
                BitAcceptanceContract {
                    case_id: TRANSPOSE_IN_PLACE_ODD_CASE_ID,
                    workload: "bit-matrix-transpose-in-place",
                    measurement: "in-place-transpose",
                    iterations: 1,
                    work_items: TRANSPOSE_SMALL_WORK_ITEMS,
                    input_bytes: TRANSPOSE_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &in_place_odd,
                },
                BitAcceptanceContract {
                    case_id: TRANSPOSE_IN_PLACE_EVEN_CASE_ID,
                    workload: "bit-matrix-transpose-in-place",
                    measurement: "in-place-transpose",
                    iterations: 2,
                    work_items: TRANSPOSE_SMALL_WORK_ITEMS,
                    input_bytes: TRANSPOSE_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &in_place_even,
                },
                BitAcceptanceContract {
                    case_id: TRANSPOSE_IN_PLACE_MAX_CASE_ID,
                    workload: "bit-matrix-transpose-in-place",
                    measurement: "in-place-transpose",
                    iterations: 1,
                    work_items: TRANSPOSE_MAX_WORK_ITEMS,
                    input_bytes: TRANSPOSE_MAX_INPUT_BYTES,
                    expected_input_digest: &max_input,
                    expected_output_digest: &in_place_max,
                },
                BitAcceptanceContract {
                    case_id: TRANSPOSE_ALLOCATING_ODD_CASE_ID,
                    workload: "bit-matrix-transpose-allocating",
                    measurement: "allocating-transpose",
                    iterations: 1,
                    work_items: TRANSPOSE_SMALL_WORK_ITEMS,
                    input_bytes: TRANSPOSE_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &allocating_odd,
                },
                BitAcceptanceContract {
                    case_id: TRANSPOSE_ALLOCATING_EVEN_CASE_ID,
                    workload: "bit-matrix-transpose-allocating",
                    measurement: "allocating-transpose",
                    iterations: 2,
                    work_items: TRANSPOSE_SMALL_WORK_ITEMS,
                    input_bytes: TRANSPOSE_SMALL_INPUT_BYTES,
                    expected_input_digest: &small_input,
                    expected_output_digest: &allocating_even,
                },
                BitAcceptanceContract {
                    case_id: TRANSPOSE_ALLOCATING_MAX_CASE_ID,
                    workload: "bit-matrix-transpose-allocating",
                    measurement: "allocating-transpose",
                    iterations: 1,
                    work_items: TRANSPOSE_MAX_WORK_ITEMS,
                    input_bytes: TRANSPOSE_MAX_INPUT_BYTES,
                    expected_input_digest: &max_input,
                    expected_output_digest: &allocating_max,
                },
            ] {
                probes.push(self.invoke_bit_acceptance(implementation, contract)?);
            }
        }
        Ok(probes)
    }

    pub(super) fn invoke_transpose_rejection_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let mut probes = Vec::with_capacity(16);
        for class in TransposeRejectionClass::all() {
            for implementation in [Implementation::Stim, Implementation::Stab] {
                let output = self.invoke_invalid_bit_width(
                    implementation,
                    class.workload(),
                    class.measurement(),
                    class.work_items(),
                )?;
                checked_transpose_rejection(&output, implementation, class)?;
                probes.push(rejected_probe(class.case_id(), implementation, &output)?);
            }
        }
        Ok(probes)
    }
}

pub(super) fn checked_transpose_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    class: TransposeRejectionClass,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = transpose_rejection_expectation(implementation, class);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::BitMatrixTransposeWorkRejection {
            implementation,
            class: class.label(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn transpose_rejection_expectation(
    implementation: Implementation,
    class: TransposeRejectionClass,
) -> (i32, &'static str) {
    match (implementation, class.failure()) {
        (Implementation::Stim, TransposeShapeFailure::BelowMinimum) => (
            2,
            "stim qualification adapter: bit-matrix transpose dimension 255 is below the minimum 256\n",
        ),
        (Implementation::Stab, TransposeShapeFailure::BelowMinimum) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nbit-matrix transpose dimension 255 is below the minimum 256\n",
        ),
        (Implementation::Stim, TransposeShapeFailure::NonSquare) => (
            2,
            "stim qualification adapter: bit-matrix transpose work count 65537 is not a perfect square\n",
        ),
        (Implementation::Stab, TransposeShapeFailure::NonSquare) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nbit-matrix transpose work count 65537 is not a perfect square\n",
        ),
        (Implementation::Stim, TransposeShapeFailure::Unaligned) => (
            2,
            "stim qualification adapter: bit-matrix transpose dimension 257 is not a multiple of 256\n",
        ),
        (Implementation::Stab, TransposeShapeFailure::Unaligned) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nbit-matrix transpose dimension 257 is not a multiple of 256\n",
        ),
        (Implementation::Stim, TransposeShapeFailure::OverCap) => (
            2,
            "stim qualification adapter: bit-matrix transpose dimension 16640 exceeds maximum 16384\n",
        ),
        (Implementation::Stab, TransposeShapeFailure::OverCap) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nbit-matrix transpose dimension 16640 exceeds maximum 16384\n",
        ),
    }
}
