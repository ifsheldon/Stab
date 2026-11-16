//! Cancellation observed inside a batch discards the batch with the random stream rewound, so a
//! session resumed for the remaining shots reproduces the uninterrupted seeded output exactly.
//!
//! The work-poll test hook lands the cancellation at a chosen poll, which makes the batch it
//! interrupts deterministic: the polls of each batch are counted on an identical session first.

use stab_model::Circuit;
use stab_records::{DetectionBatchView, DetectionSink, MeasurementBatchView, MeasurementSink};

use crate::work_poll::test_hook;
use crate::{
    DetectionSamplingCompiler, DetectionSamplingSession, LaneWidth, RandomPolicy,
    ReferenceSampleLoopPolicy, ReferenceSampleMode, SamplingCompiler, SamplingExecutionError,
    SamplingSession, Seed, ShotCount,
};

/// A short poll interval, so every batch of the fixture polls many times.
const TEST_UNITS_PER_POLL: u32 = 512;

/// Exercises every random path of a frame batch: sparse and dense Pauli noise, noisy and
/// resetting measurements, Pauli products, heralds, and classical feedback.
const NOISY: &str = "\
R 0 1 2 3 4
REPEAT 24 {
    H 0
    CX 0 1 1 2
    DEPOLARIZE1(0.02) 0 1 2 3 4
    X_ERROR(0.3) 3
    DEPOLARIZE2(0.2) 0 1 2 3
    HERALDED_ERASE(0.1) 4
    M(0.05) 0 1 2
    MPP X0*Z3 Y4
    CX rec[-1] 4
    MR 3
    DETECTOR rec[-1] rec[-5]
    DETECTOR rec[-3] rec[-4]
}
OBSERVABLE_INCLUDE(0) rec[-1]
";

/// Every delivered shot, as measurement bits or as detector bits followed by observable bits.
#[derive(Debug, Default)]
struct Shots(Vec<Vec<bool>>);

impl MeasurementSink for Shots {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.0.push(
                (0..batch.width().get())
                    .map(|bit| batch.get(shot, bit).expect("measurement bit"))
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl DetectionSink for Shots {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            let detectors = (0..batch.detector_width().get())
                .map(|bit| batch.detectors().get(shot, bit).expect("detector bit"));
            let observables = (0..batch.observable_width().get())
                .map(|bit| batch.observables().get(shot, bit).expect("observable bit"));
            self.0.push(detectors.chain(observables).collect());
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The two materializing session kinds, seen as "run some shots".
trait Session {
    /// Runs `shots` into `sink`; returns whether the run completed and the shots it delivered.
    fn run_shots(&mut self, shots: u64, sink: &mut Shots) -> (bool, u64);
    fn poisoned(&self) -> bool;
}

impl Session for SamplingSession {
    fn run_shots(&mut self, shots: u64, sink: &mut Shots) -> (bool, u64) {
        let summary = self
            .run(ShotCount::new(shots), sink)
            .expect("a cancelled sampling run is not a failure");
        let completed = summary.status() == crate::SamplingRunStatus::Completed;
        (completed, summary.committed_shots().get())
    }

    fn poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

impl Session for DetectionSamplingSession {
    fn run_shots(&mut self, shots: u64, sink: &mut Shots) -> (bool, u64) {
        let summary = self
            .run(ShotCount::new(shots), sink)
            .expect("a cancelled detection run is not a failure");
        let completed = summary.status() == crate::DetectionRunStatus::Completed;
        (completed, summary.committed_shots().get())
    }

    fn poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// Polls of each of the first `batches` batches of a fresh session.
fn polls_per_batch<S: Session>(mut session: S, lanes: u64, batches: usize) -> Vec<u64> {
    (0..batches)
        .map(|_| {
            let ((completed, _), polls) =
                test_hook::count_polls(|| session.run_shots(lanes, &mut Shots::default()));
            assert!(completed);
            polls
        })
        .collect()
}

/// Runs `prefix` shots, then the rest of `total` with cancellation landing at the middle poll of
/// the session's batch `target`, then resumes; returns everything delivered.
fn interrupted<S: Session>(
    session: impl Fn() -> S,
    lanes: u64,
    total: u64,
    prefix: u64,
    target: usize,
) -> Vec<Vec<bool>> {
    let polls = polls_per_batch(session(), lanes, target + 1);
    let first = usize::try_from(prefix.div_ceil(lanes)).expect("batch index");
    let target_polls = *polls.get(target).expect("target batch was counted");
    assert!(
        target_polls >= 2,
        "batch {target} polls only {target_polls} times"
    );
    let before = polls
        .get(first..target)
        .expect("prefix batches were counted");
    let fire_at = before.iter().sum::<u64>() + target_polls / 2;

    let mut session = session();
    let mut shots = Shots::default();
    if prefix > 0 {
        assert_eq!(session.run_shots(prefix, &mut shots), (true, prefix));
    }
    let ((completed, committed), fired) =
        test_hook::cancel_at_poll(fire_at, || session.run_shots(total - prefix, &mut shots));
    assert!(
        fired && !completed,
        "the cancellation must land inside the run"
    );
    assert_eq!(
        prefix + committed,
        target as u64 * lanes,
        "every batch before the interrupted one is delivered, nothing of it"
    );
    assert!(
        !session.poisoned(),
        "an in-batch cancellation must not poison"
    );
    let rest = total - prefix - committed;
    assert_eq!(session.run_shots(rest, &mut shots), (true, rest));
    shots.0
}

/// The interruption scenarios: inside the first batch, and inside a middle batch after a run
/// that left pending lanes.
fn assert_resumes_identically<S: Session>(session: impl Fn() -> S, lanes: u64) {
    let total = 4 * lanes + 7;
    let mut expected = Shots::default();
    assert_eq!(session().run_shots(total, &mut expected), (true, total));
    for (prefix, target) in [(0, 0), (lanes / 2 + 3, 2)] {
        let resumed = test_hook::with_units_per_poll(TEST_UNITS_PER_POLL, || {
            interrupted(&session, lanes, total, prefix, target)
        });
        assert!(
            resumed == expected.0,
            "resumed output diverged (prefix {prefix}, batch {target}, {lanes} lanes)"
        );
    }
}

fn noisy() -> Circuit {
    Circuit::from_stim_str(NOISY).expect("parse fixture")
}

#[test]
fn sampling_cancelled_inside_a_batch_resumes_byte_identically() {
    let plan = SamplingCompiler::new()
        .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
        .compile(&noisy())
        .expect("compile fixture");
    for &lane_width in LaneWidth::ALL {
        for mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            let session = || {
                plan.session_with_lane_width(RandomPolicy::Seeded(Seed::new(9)), mode, lane_width)
                    .expect("create session")
            };
            assert_resumes_identically(session, lane_width.shots_per_batch() as u64);
        }
    }
}

#[test]
fn detection_sampling_cancelled_inside_a_batch_resumes_byte_identically() {
    let plan = DetectionSamplingCompiler::new()
        .compile(&noisy())
        .expect("compile fixture");
    for &lane_width in LaneWidth::ALL {
        let session = || {
            plan.session_with_lane_width(RandomPolicy::Seeded(Seed::new(9)), lane_width)
                .expect("create session")
        };
        assert_resumes_identically(session, lane_width.shots_per_batch() as u64);
    }
}

/// A reference sample interrupted while a session is created leaves nothing cached, so the
/// next session computes it whole and samples exactly what an uninterrupted plan does.
#[test]
fn sampling_cancelled_during_the_reference_sample_retries_identically() {
    let compile = || {
        SamplingCompiler::new()
            .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
            .compile(&noisy())
            .expect("compile fixture")
    };
    let seed = RandomPolicy::Seeded(Seed::new(4));
    let mut expected = Shots::default();
    compile()
        .session(seed)
        .expect("create session")
        .run_shots(300, &mut expected);

    test_hook::with_units_per_poll(TEST_UNITS_PER_POLL, || {
        let counted = compile();
        let (created, polls) = test_hook::count_polls(|| counted.session(seed));
        created.expect("create counted session");
        assert!(polls >= 2, "the reference sample polls only {polls} times");
        let plan = compile();
        let (interrupted, fired) =
            test_hook::cancel_at_poll(polls / 2, || plan.session(seed).map(drop));
        assert!(fired);
        assert_eq!(
            interrupted,
            Err(SamplingExecutionError::Cancelled(stab_model::Cancelled))
        );
        let mut resumed = Shots::default();
        let mut session = plan.session(seed).expect("retry the reference sample");
        assert_eq!(session.run_shots(300, &mut resumed), (true, 300));
        assert!(resumed.0 == expected.0, "retried plan diverged");
    });
}

#[test]
fn single_collapse_and_fold_comparison_poll_in_chunks_and_retry_cleanly() {
    use std::fmt::Write as _;

    // Entangle 64 qubits into a cat state and measure qubit 0 once: the single random collapse
    // scans 128 rows and multiplies 64 anticommuting rows.
    let mut collapse_src = String::from("H 0\nCX");
    for q in 0..63 {
        let _ = write!(collapse_src, " {q} {}", q + 1);
    }
    collapse_src.push_str("\nM 0\n");
    let collapse_circuit = Circuit::from_stim_str(&collapse_src).expect("parse collapse circuit");

    // A foldable repeat with non-trivial stabilizers so `same_state_as` runs inner row/stabilizer
    // comparison chunks.
    let mut fold_src = String::from("H 0\nCX");
    for q in 0..63 {
        let _ = write!(fold_src, " {q} {}", q + 1);
    }
    fold_src.push_str("\nREPEAT 100 {\n    H 0\n    H 0\n    M 1\n}\n");
    let fold_circuit = Circuit::from_stim_str(&fold_src).expect("parse fold circuit");

    let seed = RandomPolicy::Seeded(Seed::new(17));
    for (circuit, policy) in [
        (&collapse_circuit, ReferenceSampleLoopPolicy::Iterate),
        (&fold_circuit, ReferenceSampleLoopPolicy::Fold),
    ] {
        let compile = || {
            SamplingCompiler::new()
                .reference_sample_loop_policy(policy)
                .compile(circuit)
                .expect("compile fixture")
        };
        let mut expected = Shots::default();
        compile()
            .session(seed)
            .expect("baseline session")
            .run_shots(64, &mut expected);

        test_hook::with_units_per_poll(64, || {
            let counted = compile();
            let (created, polls) = test_hook::count_polls(|| counted.session(seed));
            created.expect("counted session");
            assert!(polls >= 8, "expected chunked inner polls, got {polls}");
            let plan = compile();
            let (interrupted, fired) =
                test_hook::cancel_at_poll(polls - 2, || plan.session(seed).map(drop));
            assert!(fired);
            assert_eq!(
                interrupted,
                Err(SamplingExecutionError::Cancelled(stab_model::Cancelled))
            );
            let mut resumed = Shots::default();
            let mut session = plan.session(seed).expect("retry session");
            assert_eq!(session.run_shots(64, &mut resumed), (true, 64));
            assert_eq!(resumed.0, expected.0);
        });
    }
}
