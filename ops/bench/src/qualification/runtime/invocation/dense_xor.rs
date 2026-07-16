use super::{
    Implementation, InvocationError, PreparedWorkers, ProcessResult, WorkerContractProbeEvidence,
    rejected_probe,
};

pub(super) const FIRST_UNSUPPORTED_DENSE_XOR_BITS: &str = "268435712";
pub(super) const FIRST_UNALIGNED_DENSE_XOR_BITS: &str = "257";
pub(super) const FIRST_BELOW_MINIMUM_DENSE_XOR_BITS: &str = "128";
pub(super) const MAX_SUPPORTED_DENSE_XOR_BITS: u64 = 268_435_456;
pub(super) const SMALL_DENSE_XOR_BITS: u64 = 4_096;
pub(super) const ODD_DENSE_XOR_ITERATIONS: u64 = 1;
pub(super) const EVEN_DENSE_XOR_ITERATIONS: u64 = 2;
pub(super) const SMALL_DENSE_XOR_INPUT_DIGEST: &str =
    "d7fbfcc618ad7e3fd8a616be64f8b41949214afbbca6b58514d40fa5ea7ac498";
pub(super) const ODD_DENSE_XOR_OUTPUT_DIGEST: &str =
    "0a654f5fe059e663b6f2f6ddea1ab61b4fb0b85927dde926da88de95caff58d4";
pub(super) const EVEN_DENSE_XOR_OUTPUT_DIGEST: &str =
    "b6623d77b32fe22daee0e7c30fcacdf3bc332854e7dcdf7d561a0da0325a3aa3";
pub(super) const MAX_DENSE_XOR_INPUT_DIGEST: &str =
    "b3a240e29cde0478904e22b3d6d60e31f4e8c7b457d8992bab1d4d0596cc2ae0";
pub(super) const MAX_DENSE_XOR_OUTPUT_DIGEST: &str =
    "451ffe13a031a8f9656ff3e3a89c1bd224e0f1cb94193456e32ff2cd854395b8";
pub(super) const DENSE_XOR_ODD_CASE_ID: &str = "simd-bits-xor-odd";
pub(super) const DENSE_XOR_EVEN_CASE_ID: &str = "simd-bits-xor-even";
pub(super) const DENSE_XOR_MAXIMUM_CASE_ID: &str = "simd-bits-xor-maximum";
pub(super) const DENSE_XOR_CAP_CASE_ID: &str = "simd-bits-xor-over-cap";
pub(super) const DENSE_XOR_ALIGNMENT_CASE_ID: &str = "simd-bits-xor-unaligned";
pub(super) const DENSE_XOR_MINIMUM_CASE_ID: &str = "simd-bits-xor-below-minimum";

impl PreparedWorkers {
    pub(super) fn invoke_dense_xor_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-bits-xor",
            "xor-complete-vector",
            FIRST_UNSUPPORTED_DENSE_XOR_BITS,
        )?;
        checked_dense_xor_cap_rejection(&output, implementation)?;
        rejected_probe(DENSE_XOR_CAP_CASE_ID, implementation, &output)
    }

    pub(super) fn invoke_dense_xor_alignment_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-bits-xor",
            "xor-complete-vector",
            FIRST_UNALIGNED_DENSE_XOR_BITS,
        )?;
        checked_dense_xor_alignment_rejection(&output, implementation)?;
        rejected_probe(DENSE_XOR_ALIGNMENT_CASE_ID, implementation, &output)
    }

    pub(super) fn invoke_dense_xor_minimum_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-bits-xor",
            "xor-complete-vector",
            FIRST_BELOW_MINIMUM_DENSE_XOR_BITS,
        )?;
        checked_dense_xor_minimum_rejection(&output, implementation)?;
        rejected_probe(DENSE_XOR_MINIMUM_CASE_ID, implementation, &output)
    }
}

pub(super) fn checked_dense_xor_cap_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    checked_dense_xor_rejection(
        output,
        implementation,
        "over-cap",
        dense_xor_cap_rejection_expectation(implementation),
    )
}

pub(super) fn checked_dense_xor_alignment_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    checked_dense_xor_rejection(
        output,
        implementation,
        "unaligned",
        dense_xor_alignment_rejection_expectation(implementation),
    )
}

pub(super) fn checked_dense_xor_minimum_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    checked_dense_xor_rejection(
        output,
        implementation,
        "below-minimum",
        dense_xor_minimum_rejection_expectation(implementation),
    )
}

fn checked_dense_xor_rejection(
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
        return Err(InvocationError::DenseXorWidthRejection {
            implementation,
            class,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn dense_xor_cap_rejection_expectation(
    implementation: Implementation,
) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-bits-xor bit width exceeds the source-owned limit\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-bits-xor width 268435712 bits exceeds the maximum 268435456\n",
        ),
    }
}

pub(super) fn dense_xor_alignment_rejection_expectation(
    implementation: Implementation,
) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-bits-xor bit width is not a multiple of 256\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-bits-xor width 257 bits is not a multiple of 256\n",
        ),
    }
}

pub(super) fn dense_xor_minimum_rejection_expectation(
    implementation: Implementation,
) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-bits-xor bit width is below the source-owned minimum\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-bits-xor width 128 bits is below the minimum 256\n",
        ),
    }
}
