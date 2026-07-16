use super::{
    BitAcceptanceContract, Implementation, InputDigest, InvocationError, PreparedWorkers,
    ProcessResult, SemanticDigest, WorkerContractProbeEvidence, rejected_probe,
};

pub(super) const FIRST_BELOW_MINIMUM_NOT_ZERO_BITS: &str = "63";
pub(super) const FIRST_UNSUPPORTED_NOT_ZERO_BITS: &str = "268435457";
pub(super) const MAX_SUPPORTED_NOT_ZERO_BITS: u64 = 268_435_456;
pub(super) const SMALL_NOT_ZERO_BITS: u64 = 10_000;
pub(super) const SMALL_NOT_ZERO_INPUT_BYTES: u64 = 1_256;
pub(super) const NOT_ZERO_ITERATIONS: u64 = 2;
pub(super) const NOT_ZERO_EARLY_CASE_ID: &str = "simd-bits-not-zero-early";
pub(super) const NOT_ZERO_ZERO_CASE_ID: &str = "simd-bits-not-zero-all-zero";
pub(super) const NOT_ZERO_LATE_CASE_ID: &str = "simd-bits-not-zero-late";
pub(super) const NOT_ZERO_MAXIMUM_CASE_ID: &str = "simd-bits-not-zero-maximum";
pub(super) const NOT_ZERO_CAP_CASE_ID: &str = "simd-bits-not-zero-over-cap";
pub(super) const NOT_ZERO_MINIMUM_CASE_ID: &str = "simd-bits-not-zero-below-minimum";
pub(super) const SMALL_NOT_ZERO_EARLY_INPUT_DIGEST: &str =
    "652aebf153201450c8fe9d3707aed8cb0ee9fee8f5332d88e2001c56cfd0838f";
pub(super) const SMALL_NOT_ZERO_EARLY_OUTPUT_DIGEST: &str =
    "13f255827af928e2e3cf98e7379be0b49c9ab0f5c1281014016fb945d9a99ce8";
pub(super) const SMALL_NOT_ZERO_ZERO_INPUT_DIGEST: &str =
    "b6286dfe1dca80e14e17bbc6a371565900665697e8f4f2b22d30a303f804b537";
pub(super) const SMALL_NOT_ZERO_ZERO_OUTPUT_DIGEST: &str =
    "25ba7441093c190b2c669e6a68d2c190a9ea7bff8b092c5c0dfe39efd8ce1b2a";
pub(super) const SMALL_NOT_ZERO_LATE_INPUT_DIGEST: &str =
    "76618d8f234d913b3b6f99be0c83fca1e8a6eb3c5cdb6f622c06dccc7aaa2cc0";
pub(super) const SMALL_NOT_ZERO_LATE_OUTPUT_DIGEST: &str =
    "8dd09f03893d3ea3e24e3f1e4ec3b002706f7d9cefdbfafc9b82ba51bcbb5263";
pub(super) const MAX_NOT_ZERO_LATE_INPUT_DIGEST: &str =
    "6ce3c25931cb0f6aee9c2dbe7f534bfba0b5722656ef9b35d9086091d9c60472";
pub(super) const MAX_NOT_ZERO_LATE_OUTPUT_DIGEST: &str =
    "526b1acd58d6aaa5d2dd53a5edacdc0b05f37ef65076587d43739e8eb4c979bd";

impl PreparedWorkers {
    pub(super) fn invoke_not_zero_acceptance_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let early_input = InputDigest::try_new(SMALL_NOT_ZERO_EARLY_INPUT_DIGEST)?;
        let early_output = SemanticDigest::try_new(SMALL_NOT_ZERO_EARLY_OUTPUT_DIGEST)?;
        let zero_input = InputDigest::try_new(SMALL_NOT_ZERO_ZERO_INPUT_DIGEST)?;
        let zero_output = SemanticDigest::try_new(SMALL_NOT_ZERO_ZERO_OUTPUT_DIGEST)?;
        let late_input = InputDigest::try_new(SMALL_NOT_ZERO_LATE_INPUT_DIGEST)?;
        let late_output = SemanticDigest::try_new(SMALL_NOT_ZERO_LATE_OUTPUT_DIGEST)?;
        let maximum_input = InputDigest::try_new(MAX_NOT_ZERO_LATE_INPUT_DIGEST)?;
        let maximum_output = SemanticDigest::try_new(MAX_NOT_ZERO_LATE_OUTPUT_DIGEST)?;
        let mut probes = Vec::with_capacity(8);
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_bit_acceptance(
                implementation,
                BitAcceptanceContract {
                    case_id: NOT_ZERO_EARLY_CASE_ID,
                    workload: "simd-bits-not-zero-early",
                    measurement: "not-zero",
                    iterations: NOT_ZERO_ITERATIONS,
                    work_items: SMALL_NOT_ZERO_BITS,
                    input_bytes: SMALL_NOT_ZERO_INPUT_BYTES,
                    expected_input_digest: &early_input,
                    expected_output_digest: &early_output,
                },
            )?);
            probes.push(self.invoke_bit_acceptance(
                implementation,
                BitAcceptanceContract {
                    case_id: NOT_ZERO_ZERO_CASE_ID,
                    workload: "simd-bits-not-zero-zero",
                    measurement: "not-zero",
                    iterations: NOT_ZERO_ITERATIONS,
                    work_items: SMALL_NOT_ZERO_BITS,
                    input_bytes: SMALL_NOT_ZERO_INPUT_BYTES,
                    expected_input_digest: &zero_input,
                    expected_output_digest: &zero_output,
                },
            )?);
            probes.push(self.invoke_bit_acceptance(
                implementation,
                BitAcceptanceContract {
                    case_id: NOT_ZERO_LATE_CASE_ID,
                    workload: "simd-bits-not-zero-late",
                    measurement: "not-zero",
                    iterations: NOT_ZERO_ITERATIONS,
                    work_items: SMALL_NOT_ZERO_BITS,
                    input_bytes: SMALL_NOT_ZERO_INPUT_BYTES,
                    expected_input_digest: &late_input,
                    expected_output_digest: &late_output,
                },
            )?);
            probes.push(self.invoke_bit_acceptance(
                implementation,
                BitAcceptanceContract {
                    case_id: NOT_ZERO_MAXIMUM_CASE_ID,
                    workload: "simd-bits-not-zero-late",
                    measurement: "not-zero",
                    iterations: 1,
                    work_items: MAX_SUPPORTED_NOT_ZERO_BITS,
                    input_bytes: MAX_SUPPORTED_NOT_ZERO_BITS / 8,
                    expected_input_digest: &maximum_input,
                    expected_output_digest: &maximum_output,
                },
            )?);
        }
        Ok(probes)
    }

    pub(super) fn invoke_not_zero_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-bits-not-zero-late",
            "not-zero",
            FIRST_UNSUPPORTED_NOT_ZERO_BITS,
        )?;
        checked_not_zero_cap_rejection(&output, implementation)?;
        rejected_probe(NOT_ZERO_CAP_CASE_ID, implementation, &output)
    }

    pub(super) fn invoke_not_zero_minimum_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-bits-not-zero-early",
            "not-zero",
            FIRST_BELOW_MINIMUM_NOT_ZERO_BITS,
        )?;
        checked_not_zero_minimum_rejection(&output, implementation)?;
        rejected_probe(NOT_ZERO_MINIMUM_CASE_ID, implementation, &output)
    }
}

pub(super) fn checked_not_zero_cap_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    checked_not_zero_rejection(
        output,
        implementation,
        "over-cap",
        not_zero_cap_rejection_expectation(implementation),
    )
}

pub(super) fn checked_not_zero_minimum_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    checked_not_zero_rejection(
        output,
        implementation,
        "below-minimum",
        not_zero_minimum_rejection_expectation(implementation),
    )
}

fn checked_not_zero_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    class: &'static str,
    expectation: (i32, &'static str),
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = expectation;
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::NotZeroWidthRejection {
            implementation,
            class,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn not_zero_cap_rejection_expectation(
    implementation: Implementation,
) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-bits-not-zero bit width exceeds the source-owned limit\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-bits-not-zero width 268435457 bits exceeds the maximum 268435456\n",
        ),
    }
}

pub(super) fn not_zero_minimum_rejection_expectation(
    implementation: Implementation,
) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-bits-not-zero bit width is below the source-owned minimum\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-bits-not-zero width 63 bits is below the minimum 64\n",
        ),
    }
}
