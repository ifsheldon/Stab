//! Session-storage admission parity with the detection sessions.
//!
//! The Pauli-frame sampling session is the production path for every frame-admissible circuit;
//! its admission estimate must equal the bytes a constructed session retains at every lane width,
//! and the memory budget must admit the exact boundary and reject the first excess byte before
//! allocating.

use super::super::session_helpers::session_storage_bytes;
use super::*;

fn pauli_frame_plan(text: &str) -> SamplingPlan {
    budgeted_plan(text, MemoryBudget::DEFAULT)
}

fn budgeted_plan(text: &str, budget: MemoryBudget) -> SamplingPlan {
    SamplingCompiler::new()
        .memory_budget(budget)
        .compile(&Circuit::from_stim_str(text).expect("parse storage fixture"))
        .expect("compile storage fixture")
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
            for mode in [
                ReferenceSampleMode::UseReferenceSample,
                ReferenceSampleMode::SkipReferenceSample,
            ] {
                for &lane_width in LaneWidth::ALL {
                    // A fresh plan, so the first session's cached reference sample is counted.
                    let plan = pauli_frame_plan(&text);
                    let mut session = None;
                    let allocated = allocation_counter::measure(|| {
                        session = Some(
                            plan.session_with_lane_width(
                                RandomPolicy::Seeded(Seed::new(1)),
                                mode,
                                lane_width,
                            )
                            .expect("allocate Pauli-frame session"),
                        );
                    });
                    assert_eq!(
                        session_storage_bytes(&plan.inner, mode, lane_width),
                        u128::try_from(allocated.bytes_current).expect("retained session bytes"),
                        "{measurements} measurements, {} qubits, {mode:?}, {lane_width:?}",
                        plan.inner.qubit_count
                    );
                }
            }
        }
    }
}

#[test]
fn pauli_frame_session_storage_admits_the_exact_budget_and_rejects_the_first_excess_byte() {
    let text = "H 0\nCX 0 1000\nM 0 1 2 3 4 5 6 7\n";
    let mode = ReferenceSampleMode::SkipReferenceSample;
    let seeded = RandomPolicy::Seeded(Seed::new(1));
    let narrow = session_storage_bytes(&pauli_frame_plan(text).inner, mode, LaneWidth::Lanes64);
    let wide = session_storage_bytes(&pauli_frame_plan(text).inner, mode, LaneWidth::Lanes256);
    assert_eq!(
        wide,
        4 * narrow,
        "a narrow session holds a quarter of the planes"
    );
    for &lane_width in LaneWidth::ALL {
        let exact = session_storage_bytes(&pauli_frame_plan(text).inner, mode, lane_width);
        let exact_u64 = u64::try_from(exact).expect("small session");
        let at_limit = budgeted_plan(text, MemoryBudget::from_bytes(exact_u64));
        validate_session_storage(&at_limit.inner, mode, lane_width)
            .expect("admit the exact boundary without allocating its state");
        at_limit
            .session_with_lane_width(seeded, mode, lane_width)
            .expect("construct the session at the exact budget");

        let over_limit = budgeted_plan(text, MemoryBudget::from_bytes(exact_u64 - 1));
        let mut rejected = None;
        let allocated = allocation_counter::measure(|| {
            rejected = Some(
                over_limit
                    .session_with_lane_width(seeded, mode, lane_width)
                    .expect_err("public construction must use the same admission"),
            );
        });
        assert_eq!(allocated.bytes_total, 0, "{lane_width:?}");
        assert_eq!(
            rejected,
            Some(SamplingExecutionError::SessionStorageLimit {
                estimated_bytes: exact,
                limit_bytes: exact_u64 - 1,
            }),
            "{lane_width:?}"
        );
    }
    // A budget between the two estimates admits only the narrow session.
    let between = budgeted_plan(
        text,
        MemoryBudget::from_bytes(u64::try_from(narrow).expect("small")),
    );
    between
        .session_with_lane_width(seeded, mode, LaneWidth::Lanes64)
        .expect("admit the narrow session");
    between
        .session_with_lane_width(seeded, mode, LaneWidth::Lanes256)
        .expect_err("reject the wide session");
}
