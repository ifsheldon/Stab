use super::{
    BitAcceptanceContract, Implementation, InputDigest, InvocationError, PreparedWorkers,
    ProcessResult, SemanticDigest, WorkerContractProbeEvidence, rejected_probe,
};

pub(super) const SPARSE_ROW_BASE_WORK_ITEMS: u64 = 1_997;
pub(super) const SPARSE_ROW_MAX_WORK_ITEMS: u64 = SPARSE_ROW_BASE_WORK_ITEMS * 4_096;
pub(super) const SPARSE_ITEM_BASE_WORK_ITEMS: u64 = 7;
pub(super) const SPARSE_ITEM_MAX_WORK_ITEMS: u64 = SPARSE_ITEM_BASE_WORK_ITEMS * 4_096;
pub(super) const SPARSE_ROW_INPUT_BYTES: u64 = 28_008;
pub(super) const SPARSE_ITEM_INPUT_BYTES: u64 = 36;
pub(super) const SPARSE_ROW_INPUT_DIGEST: &str =
    "9fdcaf10b6a6437d51afade0e21f39acdd1130ff18255e38c0751261f93df2a2";
pub(super) const SPARSE_ITEM_INPUT_DIGEST: &str =
    "c2c1749b4bf4c7c355c1d0a8109ea53bba790034d116acea3755b533c1fb1059";
pub(super) const SPARSE_ROW_SMALL_OUTPUT_DIGEST: &str =
    "965b7771f81e6f2d3852054cd9f7264c44678c565b110239030ac3f15a0ef466";
pub(super) const SPARSE_ROW_MAX_OUTPUT_DIGEST: &str =
    "914ae143ba0e910f5a1e82fc71c02d8d71722714b508fa38c7bbfcdf267f78a7";
pub(super) const SPARSE_ITEM_SMALL_OUTPUT_DIGEST: &str =
    "ff6a52e2bae9e011bad5033d00625472e3778842ccc1065696939f48d61bba5a";
pub(super) const SPARSE_ITEM_MAX_OUTPUT_DIGEST: &str =
    "57dec6b5484ba78c84a054c42cb574a8678ff7531f219c8885d4607b7faae8ef";
pub(super) const SPARSE_ROW_SMALL_CASE_ID: &str = "sparse-xor-row-small";
pub(super) const SPARSE_ROW_MAX_CASE_ID: &str = "sparse-xor-row-maximum";
pub(super) const SPARSE_ITEM_SMALL_CASE_ID: &str = "sparse-xor-item-small";
pub(super) const SPARSE_ITEM_MAX_CASE_ID: &str = "sparse-xor-item-maximum";
pub(super) const SPARSE_ROW_PARTIAL_CASE_ID: &str = "sparse-xor-row-partial";
pub(super) const SPARSE_ROW_CAP_CASE_ID: &str = "sparse-xor-row-over-cap";
pub(super) const SPARSE_ITEM_PARTIAL_CASE_ID: &str = "sparse-xor-item-partial";
pub(super) const SPARSE_ITEM_CAP_CASE_ID: &str = "sparse-xor-item-over-cap";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SparseXorRejectionClass {
    RowPartial,
    RowOverCap,
    ItemPartial,
    ItemOverCap,
}

impl SparseXorRejectionClass {
    pub(super) const fn all() -> [Self; 4] {
        [
            Self::RowPartial,
            Self::RowOverCap,
            Self::ItemPartial,
            Self::ItemOverCap,
        ]
    }

    pub(super) const fn case_id(self) -> &'static str {
        match self {
            Self::RowPartial => SPARSE_ROW_PARTIAL_CASE_ID,
            Self::RowOverCap => SPARSE_ROW_CAP_CASE_ID,
            Self::ItemPartial => SPARSE_ITEM_PARTIAL_CASE_ID,
            Self::ItemOverCap => SPARSE_ITEM_CAP_CASE_ID,
        }
    }

    const fn workload(self) -> &'static str {
        match self {
            Self::RowPartial | Self::RowOverCap => "sparse-xor-row",
            Self::ItemPartial | Self::ItemOverCap => "sparse-xor-item",
        }
    }

    const fn measurement(self) -> &'static str {
        match self {
            Self::RowPartial | Self::RowOverCap => "row-xor",
            Self::ItemPartial | Self::ItemOverCap => "xor-item",
        }
    }

    const fn work_items(self) -> &'static str {
        match self {
            Self::RowPartial => "1998",
            Self::RowOverCap => "8181709",
            Self::ItemPartial => "8",
            Self::ItemOverCap => "28679",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::RowPartial => "row-partial",
            Self::RowOverCap => "row-over-cap",
            Self::ItemPartial => "item-partial",
            Self::ItemOverCap => "item-over-cap",
        }
    }
}

impl PreparedWorkers {
    pub(super) fn invoke_sparse_xor_acceptance_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let row_input = InputDigest::try_new(SPARSE_ROW_INPUT_DIGEST)?;
        let row_small_output = SemanticDigest::try_new(SPARSE_ROW_SMALL_OUTPUT_DIGEST)?;
        let row_max_output = SemanticDigest::try_new(SPARSE_ROW_MAX_OUTPUT_DIGEST)?;
        let item_input = InputDigest::try_new(SPARSE_ITEM_INPUT_DIGEST)?;
        let item_small_output = SemanticDigest::try_new(SPARSE_ITEM_SMALL_OUTPUT_DIGEST)?;
        let item_max_output = SemanticDigest::try_new(SPARSE_ITEM_MAX_OUTPUT_DIGEST)?;
        let mut probes = Vec::with_capacity(8);
        for implementation in [Implementation::Stim, Implementation::Stab] {
            for contract in [
                BitAcceptanceContract {
                    case_id: SPARSE_ROW_SMALL_CASE_ID,
                    workload: "sparse-xor-row",
                    measurement: "row-xor",
                    iterations: 1,
                    work_items: SPARSE_ROW_BASE_WORK_ITEMS,
                    input_bytes: SPARSE_ROW_INPUT_BYTES,
                    expected_input_digest: &row_input,
                    expected_output_digest: &row_small_output,
                },
                BitAcceptanceContract {
                    case_id: SPARSE_ROW_MAX_CASE_ID,
                    workload: "sparse-xor-row",
                    measurement: "row-xor",
                    iterations: 1,
                    work_items: SPARSE_ROW_MAX_WORK_ITEMS,
                    input_bytes: SPARSE_ROW_INPUT_BYTES,
                    expected_input_digest: &row_input,
                    expected_output_digest: &row_max_output,
                },
                BitAcceptanceContract {
                    case_id: SPARSE_ITEM_SMALL_CASE_ID,
                    workload: "sparse-xor-item",
                    measurement: "xor-item",
                    iterations: 1,
                    work_items: SPARSE_ITEM_BASE_WORK_ITEMS,
                    input_bytes: SPARSE_ITEM_INPUT_BYTES,
                    expected_input_digest: &item_input,
                    expected_output_digest: &item_small_output,
                },
                BitAcceptanceContract {
                    case_id: SPARSE_ITEM_MAX_CASE_ID,
                    workload: "sparse-xor-item",
                    measurement: "xor-item",
                    iterations: 1,
                    work_items: SPARSE_ITEM_MAX_WORK_ITEMS,
                    input_bytes: SPARSE_ITEM_INPUT_BYTES,
                    expected_input_digest: &item_input,
                    expected_output_digest: &item_max_output,
                },
            ] {
                probes.push(self.invoke_bit_acceptance(implementation, contract)?);
            }
        }
        Ok(probes)
    }

    pub(super) fn invoke_sparse_xor_rejection_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let mut probes = Vec::with_capacity(8);
        for class in SparseXorRejectionClass::all() {
            for implementation in [Implementation::Stim, Implementation::Stab] {
                let output = self.invoke_invalid_bit_width(
                    implementation,
                    class.workload(),
                    class.measurement(),
                    class.work_items(),
                )?;
                checked_sparse_xor_rejection(&output, implementation, class)?;
                probes.push(rejected_probe(class.case_id(), implementation, &output)?);
            }
        }
        Ok(probes)
    }
}

pub(super) fn checked_sparse_xor_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    class: SparseXorRejectionClass,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        sparse_xor_rejection_expectation(implementation, class);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::SparseXorWorkRejection {
            implementation,
            class: class.label(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn sparse_xor_rejection_expectation(
    implementation: Implementation,
    class: SparseXorRejectionClass,
) -> (i32, &'static str) {
    match (implementation, class) {
        (Implementation::Stim, SparseXorRejectionClass::RowPartial) => (
            2,
            "stim qualification adapter: sparse-xor-row work count 1998 is not a positive multiple of 1997\n",
        ),
        (Implementation::Stab, SparseXorRejectionClass::RowPartial) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsparse-xor-row work count 1998 is not a positive multiple of 1997\n",
        ),
        (Implementation::Stim, SparseXorRejectionClass::RowOverCap) => (
            2,
            "stim qualification adapter: sparse-xor-row work count 8181709 exceeds maximum 8179712\n",
        ),
        (Implementation::Stab, SparseXorRejectionClass::RowOverCap) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsparse-xor-row work count 8181709 exceeds maximum 8179712\n",
        ),
        (Implementation::Stim, SparseXorRejectionClass::ItemPartial) => (
            2,
            "stim qualification adapter: sparse-xor-item work count 8 is not a positive multiple of 7\n",
        ),
        (Implementation::Stab, SparseXorRejectionClass::ItemPartial) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsparse-xor-item work count 8 is not a positive multiple of 7\n",
        ),
        (Implementation::Stim, SparseXorRejectionClass::ItemOverCap) => (
            2,
            "stim qualification adapter: sparse-xor-item work count 28679 exceeds maximum 28672\n",
        ),
        (Implementation::Stab, SparseXorRejectionClass::ItemOverCap) => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsparse-xor-item work count 28679 exceeds maximum 28672\n",
        ),
    }
}
