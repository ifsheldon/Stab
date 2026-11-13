//! Session-storage admission parity with the detection sessions.
//!
//! The Pauli-frame sampling session is the production path for every frame-admissible circuit;
//! its admission estimate must equal the bytes a constructed session retains, and the 256 MiB
//! limit must admit the exact boundary and reject the first excess byte before allocating.

use super::super::session_helpers::session_storage_bytes;
use super::*;

fn pauli_frame_plan(text: &str) -> SamplingPlan {
    let plan = SamplingCompiler::new()
        .compile(&Circuit::from_stim_str(text).expect("parse storage fixture"))
        .expect("compile storage fixture");
    assert!(
        matches!(plan.inner.kind, SamplingPlanKind::PauliFrame),
        "storage fixture must select the Pauli-frame session: {text}"
    );
    plan
}

#[test]
fn pauli_frame_session_storage_estimate_equals_retained_allocation_at_word_boundaries() {
    for measurements in [2_usize, 63, 64, 65, 255, 256, 257] {
        for extra_qubits in [0_usize, 1, 64] {
            let targets = (0..measurements)
                .map(|qubit| qubit.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!("H 0\nCX 0 {}\nM {targets}\n", measurements + extra_qubits);
            let plan = pauli_frame_plan(&text);
            for mode in [
                ReferenceSampleMode::UseReferenceSample,
                ReferenceSampleMode::SkipReferenceSample,
            ] {
                let mut session = None;
                let allocated = allocation_counter::measure(|| {
                    session = Some(
                        plan.session_with_reference_mode(RandomPolicy::Seeded(Seed::new(1)), mode)
                            .expect("allocate Pauli-frame session"),
                    );
                });
                assert_eq!(
                    session_storage_bytes(&plan.inner, mode),
                    u128::try_from(allocated.bytes_current).expect("retained session bytes"),
                    "{measurements} measurements, {} qubits, {mode:?}",
                    plan.inner.qubit_count
                );
            }
        }
    }
}

#[test]
fn pauli_frame_session_storage_rejects_first_excess_before_allocation() {
    // Without a stored reference, eight measurements retain 8 * (8 + 32) bytes of batch and
    // measurement planes and each qubit retains two 32-byte frame words, so 4,194,299 qubits
    // put the session exactly at 256 MiB.
    let measured = "M 4194291 4194292 4194293 4194294 4194295 4194296 4194297 4194298\n";
    let at_limit = pauli_frame_plan(measured);
    let mode = ReferenceSampleMode::SkipReferenceSample;
    validate_session_storage(&at_limit.inner, mode)
        .expect("admit the exact boundary without allocating its state");
    assert_eq!(
        session_storage_bytes(&at_limit.inner, mode),
        u128::from(MAX_SAMPLING_SESSION_STORAGE_BYTES)
    );

    let over_limit = pauli_frame_plan(&format!("{measured}H 4194299\n"));
    // Check admission first so a regression cannot allocate the large frame in this test.
    validate_session_storage(&over_limit.inner, mode)
        .expect_err("reject the first excess qubit before attempting construction");
    let mut rejected = None;
    let allocated = allocation_counter::measure(|| {
        rejected = Some(
            over_limit
                .session_with_reference_mode(RandomPolicy::Seeded(Seed::new(1)), mode)
                .expect_err("public construction must use the same admission"),
        );
    });
    assert_eq!(allocated.bytes_total, 0);
    assert_eq!(
        rejected,
        Some(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes: u128::from(MAX_SAMPLING_SESSION_STORAGE_BYTES) + 64,
            limit_bytes: MAX_SAMPLING_SESSION_STORAGE_BYTES,
        })
    );
}
