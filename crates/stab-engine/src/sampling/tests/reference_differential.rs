//! Reference-sample differentials and properties for the inverse-tableau reference simulator.
//!
//! The earlier generator-list simulator, iterating every repeat, is the semantic baseline. The
//! inverse-tableau simulator must reproduce it exactly with and without repeat folding,
//! including on loops whose iterations change measurement signs, cycle with a period longer than
//! one, act on entangled state, or use every Clifford gate, reset, measurement basis, pair
//! measurement, and Pauli product in the catalog.

use rand::RngExt as _;
use stab_analysis::{ColorCodeTask, RepetitionCodeTask};
use stab_model::GateTargetGroupKind;

use super::*;

struct ReferencePaths {
    baseline: Vec<bool>,
    baseline_folded: Vec<bool>,
    iterated: Vec<bool>,
    folded: Vec<bool>,
    folded_stats: ReferenceExecutionStats,
    product: Vec<bool>,
}

impl ReferencePaths {
    fn compute(circuit: &Circuit) -> Self {
        let iterated_plan = SamplingCompiler::new()
            .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
            .compile(circuit)
            .expect("compile iterated reference plan");
        let folded_plan = SamplingCompiler::new()
            .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Fold)
            .compile(circuit)
            .expect("compile folded reference plan");
        let baseline =
            super::super::api::compute_stabilizer_frame_reference_sample(&iterated_plan.inner)
                .expect("iterated generator-list reference");
        let baseline_folded =
            super::super::api::compute_stabilizer_frame_reference_sample(&folded_plan.inner)
                .expect("folded generator-list reference");
        let (iterated, _) = reference_with_stats(&iterated_plan);
        let (folded, folded_stats) = reference_with_stats(&folded_plan);
        let product = bits(
            &folded_plan
                .try_reference_sample()
                .expect("product reference sample"),
        );
        Self {
            baseline,
            baseline_folded,
            iterated,
            folded,
            folded_stats,
            product,
        }
    }

    fn assert_agree(&self, label: &str) {
        assert_eq!(
            self.baseline_folded, self.baseline,
            "generator-list folded vs iterated: {label}"
        );
        assert_eq!(
            self.iterated, self.baseline,
            "inverse tableau iterated: {label}"
        );
        assert_eq!(
            self.folded, self.baseline,
            "inverse tableau folded: {label}"
        );
        assert_eq!(self.product, self.baseline, "product reference: {label}");
    }
}

fn parse(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse reference differential circuit")
}

fn cycle(pattern: &[bool], repetitions: usize) -> Vec<bool> {
    pattern
        .iter()
        .copied()
        .cycle()
        .take(pattern.len() * repetitions)
        .collect()
}

#[test]
fn folded_references_match_iteration_on_sign_changing_periodic_and_entangled_loops() {
    // (label, circuit, hand-derived reference, whether the fold path must reuse iterations)
    let cases: [(&str, &str, Vec<bool>, bool); 9] = [
        (
            "sign-changing single qubit",
            "REPEAT 100 {\n    X 0\n    M 0\n}\n",
            cycle(&[true, false], 50),
            false,
        ),
        (
            "sign-restoring single qubit",
            "REPEAT 100 {\n    X 0\n    M 0\n    X 0\n}\n",
            vec![true; 100],
            true,
        ),
        (
            "period-4 counter",
            "REPEAT 100 {\n    X 0\n    CX 0 1\n    M 0 1\n}\n",
            cycle(&[true, true, false, true, true, false, false, false], 25),
            false,
        ),
        (
            "sign-changing Bell parity",
            "H 0\nCX 0 1\nREPEAT 100 {\n    X 0\n    R 2\n    CX 0 2 1 2\n    M 2\n}\n",
            cycle(&[true, false], 50),
            false,
        ),
        (
            "sign-restoring Bell parity",
            "H 0\nCX 0 1\nREPEAT 100 {\n    X 0\n    CX 0 2 1 2\n    M 2\n    CX 0 2 1 2\n    X 0\n}\n",
            vec![true; 100],
            true,
        ),
        (
            "invariant uncomputed Bell parity",
            "H 0\nCX 0 1\nREPEAT 100 {\n    CX 0 2 1 2\n    M 2\n    CX 0 2 1 2\n}\n",
            vec![false; 100],
            true,
        ),
        (
            "invariant reset Bell parity",
            "H 0\nCX 0 1\nREPEAT 100 {\n    R 2\n    CX 0 2 1 2\n    M 2\n}\n",
            vec![false; 100],
            false,
        ),
        (
            "invariant measure-reset Bell parity",
            "H 0\nCX 0 1\nREPEAT 100 {\n    CX 0 2 1 2\n    MR 2\n}\n",
            vec![false; 100],
            false,
        ),
        (
            "random outcomes collapse to the canonical branch",
            "REPEAT 100 {\n    H 0\n    CX 0 1\n    M 0 1\n    R 0 1\n}\n",
            vec![false; 200],
            false,
        ),
    ];
    for (label, text, expected, must_fold) in cases {
        let paths = ReferencePaths::compute(&parse(text));
        assert_eq!(paths.baseline, expected, "baseline: {label}");
        paths.assert_agree(label);
        if must_fold {
            assert_eq!(
                paths.folded_stats.folded_repeats, 1,
                "{label} must exercise the fold path"
            );
        }
    }
}

#[test]
fn folded_reference_matches_iteration_on_surface_code_with_inserted_paulis() {
    let params = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ),
        CodeDistance::try_new(3).expect("valid code distance"),
        RoundCount::try_new(25).expect("valid round count"),
    )
    .expect("valid surface-code parameters");
    let generated = params.generate().expect("generate surface circuit");
    let text = generated.circuit().to_stim_string();
    let baseline = ReferencePaths::compute(generated.circuit());
    baseline.assert_agree("surface code");

    let final_data_qubit = text
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("M "))
        .and_then(|targets| targets.split_whitespace().next())
        .expect("final data measurement");
    let body_start = text
        .find("REPEAT")
        .and_then(|start| Some(start + text.get(start..)?.find('\n')? + 1))
        .expect("surface round loop header");
    for inserted in [
        format!("    X {final_data_qubit}\n"),
        format!("    Y {final_data_qubit}\n"),
        format!("    Z {final_data_qubit}\n    X {final_data_qubit}\n"),
    ] {
        let mut modified = text.clone();
        modified.insert_str(body_start, &inserted);
        let paths = ReferencePaths::compute(&parse(&modified));
        let label = format!("surface code with {}", inserted.trim());
        paths.assert_agree(&label);
        assert_ne!(
            paths.baseline, baseline.baseline,
            "{label}: the inserted Pauli must change reference signs"
        );
    }
}

/// Emits random qubit-targeted instructions covering the whole Clifford catalog, resets and
/// measurements in every basis, pair measurements, Pauli products, feedback, and ignored noise.
pub(super) struct RandomCliffordEmitter {
    single_qubit_gates: Vec<&'static str>,
    two_qubit_gates: Vec<&'static str>,
    product_gates: Vec<&'static str>,
    measurements: usize,
}

impl RandomCliffordEmitter {
    pub(super) fn new() -> Self {
        let mut single_qubit_gates = Vec::new();
        let mut two_qubit_gates = Vec::new();
        let mut product_gates = Vec::new();
        for gate in Gate::all().filter(|gate| gate.is_unitary()) {
            match gate.target_group_kind() {
                GateTargetGroupKind::Singles => single_qubit_gates.push(gate.canonical_name()),
                GateTargetGroupKind::Pairs => two_qubit_gates.push(gate.canonical_name()),
                GateTargetGroupKind::PauliProducts => product_gates.push(gate.canonical_name()),
                GateTargetGroupKind::None | GateTargetGroupKind::AllTargets => {}
            }
        }
        assert!(single_qubit_gates.len() >= 20, "{single_qubit_gates:?}");
        assert!(two_qubit_gates.len() >= 20, "{two_qubit_gates:?}");
        assert!(!product_gates.is_empty());
        Self {
            single_qubit_gates,
            two_qubit_gates,
            product_gates,
            measurements: 0,
        }
    }

    pub(super) fn emit(
        &mut self,
        text: &mut String,
        rng: &mut SmallRng,
        qubits: usize,
        operations: usize,
    ) {
        for _ in 0..operations {
            let qubit = rng.random_range(0..qubits);
            let other = (qubit + rng.random_range(1..qubits.max(2))) % qubits;
            let invert = if rng.random_bool(0.25) { "!" } else { "" };
            let choice = rng.random_range(0..20);
            let line = match choice {
                0..=5 => {
                    let gate = pick(rng, &self.single_qubit_gates);
                    format!("{gate} {qubit}")
                }
                6..=10 if qubits > 1 => {
                    let gate = pick(rng, &self.two_qubit_gates);
                    format!("{gate} {qubit} {other}")
                }
                11 => format!("{} {qubit}", pick(rng, &["R", "RX", "RY"])),
                12 | 13 => {
                    self.measurements += 1;
                    format!("{} {invert}{qubit}", pick(rng, &["M", "MX", "MY"]))
                }
                14 => {
                    self.measurements += 1;
                    format!("{} {invert}{qubit}", pick(rng, &["MR", "MRX", "MRY"]))
                }
                15 if qubits > 1 => {
                    self.measurements += 1;
                    let gate = pick(rng, &["MXX", "MYY", "MZZ"]);
                    format!("{gate} {invert}{qubit} {other}")
                }
                16 | 17 => {
                    let product = random_product(rng, qubits);
                    if choice == 16 {
                        self.measurements += 1;
                        format!("MPP {invert}{product}")
                    } else {
                        format!("{} {product}", pick(rng, &self.product_gates))
                    }
                }
                18 if self.measurements > 0 => {
                    let lookback = rng.random_range(1..=self.measurements.min(4));
                    let gate = pick(rng, &["CX", "CY", "CZ"]);
                    format!("{gate} rec[-{lookback}] {qubit}")
                }
                _ => format!("X_ERROR(0.25) {qubit}"),
            };
            text.push_str(&line);
            text.push('\n');
        }
    }
}

fn pick<'a>(rng: &mut SmallRng, values: &[&'a str]) -> &'a str {
    values
        .get(rng.random_range(0..values.len()))
        .copied()
        .expect("non-empty choice list")
}

fn random_product(rng: &mut SmallRng, qubits: usize) -> String {
    let length = rng.random_range(1..=qubits.min(4));
    let mut chosen: Vec<usize> = Vec::new();
    while chosen.len() < length {
        let qubit = rng.random_range(0..qubits);
        if !chosen.contains(&qubit) {
            chosen.push(qubit);
        }
    }
    chosen
        .iter()
        .map(|qubit| format!("{}{qubit}", pick(rng, &["X", "Y", "Z"])))
        .collect::<Vec<_>>()
        .join("*")
}

pub(super) fn random_clifford_circuit(rng: &mut SmallRng, qubits: usize) -> String {
    let mut emitter = RandomCliffordEmitter::new();
    let mut text = String::new();
    emitter.emit(&mut text, rng, qubits, 2 * qubits.min(64) + 20);
    let repetitions = if qubits <= 8 { 40 } else { 3 };
    text.push_str(&format!("REPEAT {repetitions} {{\n"));
    emitter.emit(&mut text, rng, qubits, qubits.min(48) + 10);
    text.push_str("}\n");
    // A product over a wide support exercises multi-word rows and collapses.
    let wide = (0..qubits)
        .step_by(qubits.div_ceil(6).max(1))
        .map(|qubit| format!("X{qubit}"))
        .collect::<Vec<_>>()
        .join("*");
    writeln!(text, "MPP {wide}").expect("write wide product");
    for basis in ["MX", "MY", "M"] {
        text.push_str(basis);
        for qubit in 0..qubits {
            write!(text, " {qubit}").expect("write final measurement");
        }
        text.push('\n');
    }
    text
}

#[test]
fn inverse_tableau_matches_generator_baseline_on_random_clifford_circuits() {
    let mut rng = SmallRng::seed_from_u64(0xC11F_F04D);
    super::super::reference_simulator::take_pivot_column_stats();
    for qubits in [1_usize, 2, 3, 5, 8, 17, 63, 64, 65, 127, 128, 129, 300] {
        for _ in 0..3 {
            let text = random_clifford_circuit(&mut rng, qubits);
            let paths = ReferencePaths::compute(&parse(&text));
            paths.assert_agree(&format!("{qubits}-qubit random Clifford circuit:\n{text}"));
        }
    }
    // The wide final measurement layers collapse through the transposed pivot columns, and
    // their collapses update the copy both bit by bit and by row mask.
    let stats = super::super::reference_simulator::take_pivot_column_stats();
    assert!(stats.builds > 0, "{stats:?}");
    assert!(stats.bit_follows > 0, "{stats:?}");
    assert!(stats.mask_follows > 0, "{stats:?}");
}

#[test]
fn pivot_columns_survive_interleaved_gates_resets_and_sign_updates() {
    // Wide random layers whose collapses build the transposed pivot columns, interleaved with
    // gates that discard them, reset corrections and Paulis that keep them, and feedback, in
    // every basis and inside a folded repeat.
    let mut rng = SmallRng::seed_from_u64(0x9_1707);
    let qubits = 150_usize;
    let all = (0..qubits)
        .map(|qubit| qubit.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let mut text = String::new();
    for gate in ["MR", "MX", "MRY", "M", "MY", "MRX"] {
        writeln!(text, "H {all}").expect("write layer");
        for _ in 0..qubits {
            let control = rng.random_range(0..qubits);
            let target = (control + rng.random_range(1..qubits)) % qubits;
            writeln!(text, "CX {control} {target}").expect("write entangler");
        }
        let half = (0..qubits)
            .step_by(2)
            .map(|qubit| qubit.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(text, "{gate} {half}\nX 1 3\nCX rec[-1] 5\nM {all}").expect("write measurements");
    }
    text.push_str("REPEAT 20 {\n    H 0 2 4\n    CX 0 1 2 3 4 5\n    MX ");
    text.push_str(&all);
    text.push_str("\n}\n");
    super::super::reference_simulator::take_pivot_column_stats();
    let paths = ReferencePaths::compute(&parse(&text));
    paths.assert_agree("interleaved wide collapse layers");
    let stats = super::super::reference_simulator::take_pivot_column_stats();
    assert!(stats.builds > 6, "{stats:?}");
}

#[test]
fn inverse_tableau_matches_generator_baseline_above_one_thousand_qubits() {
    let mut rng = SmallRng::seed_from_u64(0x1100);
    let qubits = 1100;
    let mut emitter = RandomCliffordEmitter::new();
    let mut text = String::new();
    // Entangle all qubits first so later collapses touch many multi-word rows.
    for qubit in (0..qubits).step_by(2) {
        writeln!(text, "H {qubit}").expect("write layer");
    }
    for qubit in 0..qubits - 1 {
        writeln!(text, "CX {qubit} {}", qubit + 1).expect("write chain");
    }
    emitter.emit(&mut text, &mut rng, qubits, 400);
    text.push_str("MX");
    for qubit in 0..qubits {
        write!(text, " {qubit}").expect("write final measurement");
    }
    text.push('\n');
    let circuit = parse(&text);
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile wide circuit");
    assert_eq!(plan.qubit_count(), qubits);
    super::super::reference_simulator::take_pivot_column_stats();
    assert_eq!(
        bits(
            &plan
                .try_reference_sample()
                .expect("inverse-tableau reference")
        ),
        super::super::api::compute_stabilizer_frame_reference_sample(&plan.inner)
            .expect("generator-list reference"),
    );
    assert!(super::super::reference_simulator::take_pivot_column_stats().builds > 0);
}

/// Rewrites the last top-level `REPEAT n` loop as `REPEAT outer { REPEAT n / outer { .. } }`
/// followed by the remaining `n % outer` iterations, preserving the circuit's semantics.
fn nest_round_loop(text: &str, outer: u64) -> String {
    let start = text.rfind("\nREPEAT ").expect("generated round loop") + 1;
    let header_end = start
        + text
            .get(start..)
            .and_then(|rest| rest.find('\n'))
            .expect("header");
    let count: u64 = text
        .get(start + "REPEAT ".len()..header_end)
        .and_then(|header| header.trim_end_matches(" {").parse().ok())
        .expect("repeat count");
    let mut depth = 1_usize;
    let mut close = header_end;
    for (offset, line) in text
        .get(header_end + 1..)
        .expect("body")
        .split_inclusive('\n')
        .scan(header_end + 1, |position, line| {
            let line_start = *position;
            *position += line.len();
            Some((line_start, line))
        })
    {
        if line.trim_end().ends_with('{') {
            depth += 1;
        } else if line.trim() == "}" {
            depth -= 1;
            if depth == 0 {
                close = offset;
                break;
            }
        }
    }
    let body = text.get(header_end + 1..close).expect("round loop body");
    let after = close
        + text
            .get(close..)
            .and_then(|rest| rest.find('\n'))
            .expect("close")
        + 1;
    let mut nested = format!(
        "{}REPEAT {outer} {{\nREPEAT {} {{\n{body}}}\n}}\n",
        text.get(..start).expect("prefix"),
        count / outer,
    );
    if !count.is_multiple_of(outer) {
        nested.push_str(&format!("REPEAT {} {{\n{body}}}\n", count % outer));
    }
    nested.push_str(text.get(after..).expect("suffix"));
    nested
}

#[test]
fn reference_samples_satisfy_every_deterministic_detector_with_folded_and_nested_repeats() {
    let rounds = RoundCount::try_new(13).expect("valid round count");
    let distance = CodeDistance::try_new(5).expect("valid code distance");
    let mut circuits = Vec::new();
    for task in [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
        SurfaceCodeTask::UnrotatedMemoryX,
        SurfaceCodeTask::UnrotatedMemoryZ,
    ] {
        let params = CodeCircuitParams::try_new(CodeTask::Surface(task), distance, rounds)
            .expect("surface parameters");
        circuits.push(
            params
                .generate()
                .expect("generate surface circuit")
                .circuit()
                .to_stim_string(),
        );
    }
    let color =
        CodeCircuitParams::try_new(CodeTask::Color(ColorCodeTask::MemoryXyz), distance, rounds)
            .expect("color parameters");
    circuits.push(
        color
            .generate()
            .expect("generate color circuit")
            .circuit()
            .to_stim_string(),
    );
    let repetition = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        distance,
        rounds,
    )
    .expect("repetition parameters");
    circuits.push(
        repetition
            .generate()
            .expect("generate repetition circuit")
            .circuit()
            .to_stim_string(),
    );

    let mut rng = SmallRng::seed_from_u64(0xDE7E_C70D);
    for text in circuits {
        let nested = nest_round_loop(&text, 3);
        assert!(nested.contains("REPEAT 3 {\nREPEAT "), "{nested}");
        let mut flat_reference = None;
        for variant in [text.clone(), nested] {
            let circuit = parse(&variant);
            let plan = SamplingCompiler::new()
                .compile(&circuit)
                .expect("compile detector circuit");
            let reference = bits(&plan.try_reference_sample().expect("reference sample"));
            // Nesting the round loop must not change the measurement record.
            assert_eq!(
                flat_reference.get_or_insert_with(|| reference.clone()),
                &reference,
                "{variant}"
            );
            let expected = detector_parities(&circuit, &reference);
            assert!(!expected.is_empty(), "{variant}");
            let signs = crate::circuit_reference_signs(&circuit).expect("reference signs");
            assert_eq!(
                signs.detector_signs().iter().collect::<Vec<_>>(),
                expected,
                "{variant}"
            );
            // Independent noiseless executions draw every random outcome, so each deterministic
            // detector must take the same value in them as in the reference sample.
            let mut frame = StabilizerFrame::try_new(plan.qubit_count()).expect("sampling frame");
            let (mut record, mut output) = (Vec::new(), Vec::new());
            for _ in 0..3 {
                plan.sample_shot_in_mode_into(
                    &mut rng,
                    ExecutionMode::Sample,
                    &[],
                    &mut frame,
                    &mut record,
                    &mut output,
                )
                .expect("sample noiseless shot");
                assert_eq!(
                    detector_parities(&circuit, &output),
                    expected,
                    "a deterministic detector disagrees with the reference sample:\n{variant}"
                );
            }
            ReferencePaths::compute(&circuit).assert_agree(&variant);
        }
    }
}

/// Detector parities of one measurement record, by walking the flattened circuit.
fn detector_parities(circuit: &Circuit, record: &[bool]) -> Vec<bool> {
    let mut measured = 0_usize;
    let mut parities = Vec::new();
    for instruction in circuit.iter_flattened_instructions() {
        let gate = instruction.gate();
        if gate.canonical_name() == "DETECTOR" {
            let parity = instruction
                .targets()
                .iter()
                .filter_map(stab_model::Target::measurement_record_offset)
                .fold(false, |parity, offset| {
                    let index = i64::try_from(measured)
                        .ok()
                        .and_then(|measured| measured.checked_add(i64::from(offset.get())))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("detector lookback within record");
                    parity ^ record.get(index).copied().expect("recorded measurement")
                });
            parities.push(parity);
        } else if gate.produces_measurements() {
            measured += instruction.target_groups().len();
        }
    }
    assert_eq!(measured, record.len(), "measurement count");
    parities
}

#[test]
fn sessions_share_the_plan_reference_instead_of_recomputing_it() {
    let params = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX),
        CodeDistance::try_new(5).expect("valid code distance"),
        RoundCount::try_new(5).expect("valid round count"),
    )
    .expect("valid surface-code parameters")
    .with_before_measure_flip_probability(Probability::try_new(0.01).expect("probability"));
    let generated = params.generate().expect("generate surface circuit");
    let plan = SamplingCompiler::new()
        .compile(generated.circuit())
        .expect("compile surface circuit");

    let runs_before = super::super::reference_run::reference_run_count();
    let sample = |plan: &SamplingPlan| {
        let mut session = plan
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("session");
        let mut sink = RecordSink::default();
        session.run(ShotCount::new(70), &mut sink).expect("run");
        sink.records
    };
    let first = sample(&plan);
    assert_eq!(
        super::super::reference_run::reference_run_count(),
        runs_before + 1,
        "the first session computes the reference"
    );
    let second = sample(&plan.clone());
    let reference = bits(&plan.try_reference_sample().expect("reference"));
    assert_eq!(
        super::super::reference_run::reference_run_count(),
        runs_before + 1,
        "later sessions and reference requests reuse the plan's reference"
    );
    assert_eq!(first, second, "equal seeds must give equal samples");
    assert_eq!(
        reference,
        super::super::api::compute_stabilizer_frame_reference_sample(&plan.inner)
            .expect("generator-list reference")
    );
}

#[test]
fn memory_experiment_rounds_fold_once_the_state_recurs() {
    let probability = Probability::try_new(0.001).expect("valid generated-circuit noise");
    for task in [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
    ] {
        let params = CodeCircuitParams::try_new(
            CodeTask::Surface(task),
            CodeDistance::try_new(5).expect("valid code distance"),
            RoundCount::try_new(60).expect("valid round count"),
        )
        .expect("valid surface-code parameters")
        .with_after_clifford_depolarization(probability);
        let generated = params.generate().expect("generate surface circuit");
        let paths = ReferencePaths::compute(generated.circuit());
        paths.assert_agree(&format!("{task:?}"));
        // The round's Clifford does not return the preparing tableau to itself, so this fold
        // relies on comparing stabilizer states rather than tableaux.
        assert_eq!(paths.folded_stats.folded_repeats, 1, "{task:?}");
        assert_eq!(paths.folded_stats.reused_iterations, 58, "{task:?}");
    }
}

/// Qubit registers whose joint state cycles through a chosen transient and period.
///
/// Each register is a row of qubits holding one excitation, so its states are distinct
/// computational basis states, and every register measures a qubit each iteration so the output
/// depends on the cycle phase:
///
/// - A conveyor of `transient` qubits starts with its excitation on the first qubit. Each
///   iteration first resets the last qubit, then moves the excitation one qubit along by swapping
///   from the far end inward, and measures the last qubit. After `transient - 1` iterations the
///   excitation sits on the last qubit, the next iteration resets it, and from then on the
///   register stays empty: transient `transient`, period 1.
/// - A rotor of `period` qubits starts with its excitation on the first qubit. The same swap
///   sweep rotates the excitation cyclically, and the first qubit is measured: transient 0,
///   period `period`. A helper qubit copies the first rotor qubit and is measured in the X basis,
///   so every iteration also collapses a random outcome whose state depends on the phase.
/// - A Bell pair with a parity ancilla is measured and reset each iteration. Its tableau changes
///   while its state does not, so recurrences need the stabilizer-state comparison.
///
/// The joint state has transient `transient` (0 without a conveyor) and period `period`
/// (1 without a rotor).
struct CycleRegisters {
    prefix: String,
    body: String,
    next_qubit: usize,
}

impl CycleRegisters {
    fn new(transient: usize, period: usize) -> Self {
        let mut registers = Self {
            prefix: String::new(),
            body: String::new(),
            next_qubit: 0,
        };
        let bell = registers.take(3);
        let (left, right, parity) = (bell, bell + 1, bell + 2);
        writeln!(registers.prefix, "H {left}\nCX {left} {right}").expect("bell");
        writeln!(
            registers.body,
            "CX {left} {parity} {right} {parity}\nMR {parity}"
        )
        .expect("bell parity");
        if transient > 0 {
            let first = registers.take(transient);
            let last = first + transient - 1;
            writeln!(registers.prefix, "X {first}").expect("conveyor start");
            writeln!(registers.body, "R {last}").expect("conveyor reset");
            registers.sweep(first, transient);
            writeln!(registers.body, "M {last}").expect("conveyor measurement");
        }
        if period > 1 {
            let first = registers.take(period);
            let helper = registers.take(1);
            writeln!(registers.prefix, "X {first}").expect("rotor start");
            registers.sweep(first, period);
            writeln!(
                registers.body,
                "M {first}\nCX {first} {helper}\nMX {helper}\nR {helper}"
            )
            .expect("rotor measurements");
        }
        registers
    }

    /// Allocates `count` consecutive qubits and returns the first.
    fn take(&mut self, count: usize) -> usize {
        let first = self.next_qubit;
        self.next_qubit += count;
        first
    }

    /// Swaps neighbours of `first..first + len` from the far end inward, which moves every
    /// excitation one qubit along and wraps the last qubit's excitation to the first.
    fn sweep(&mut self, first: usize, len: usize) {
        for qubit in (first..first + len.saturating_sub(1)).rev() {
            writeln!(self.body, "SWAP {qubit} {}", qubit + 1).expect("sweep");
        }
    }

    fn circuit(&self, count: u64) -> String {
        format!("{}REPEAT {count} {{\n{}}}\nM 0 1\n", self.prefix, self.body)
    }
}

/// Runs `test` with cost-independent fold probing, restoring the default afterwards.
fn with_forced_probes(test: impl FnOnce()) {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            super::super::reference_fold::force_fold_probes(false);
        }
    }
    let _restore = Restore;
    super::super::reference_fold::force_fold_probes(true);
    test();
}

#[test]
fn folds_cycles_with_transients_and_periods_at_every_remainder() {
    with_forced_probes(|| {
        for (transient, period) in [
            (1, 1),
            (2, 1),
            (5, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (3, 3),
            (2, 4),
            (5, 2),
        ] {
            let registers = CycleRegisters::new(transient, period);
            // Brent's schedule finds the cycle within 2 * max(transient, period) + period
            // iterations; the following counts leave every remainder modulo the period.
            for count in (40..40 + period as u64).chain([2, 3]) {
                let text = registers.circuit(count);
                let label = format!("transient {transient}, period {period}, count {count}");
                let paths = ReferencePaths::compute(&parse(&text));
                paths.assert_agree(&format!("{label}:\n{text}"));
                if count >= 40 {
                    assert_eq!(paths.folded_stats.folded_repeats, 1, "{label}");
                    assert_eq!(paths.folded_stats.longest_period, period as u64, "{label}");
                    assert!(paths.folded_stats.reused_iterations >= 20, "{label}");
                }
            }
        }
    });
}

#[test]
fn folds_nested_repeats_at_both_levels() {
    with_forced_probes(|| {
        // The inner rotor has period 3 and runs a multiple of 3 iterations, so each outer
        // iteration returns it to its start; the outer conveyor then sets a transient of 2 and
        // the outer X toggle a period of 2.
        let inner = CycleRegisters::new(0, 3);
        let toggle = inner.next_qubit + 2;
        let text = format!(
            "{prefix}X {conveyor}\nREPEAT 45 {{\nREPEAT 30 {{\n{body}}}\nR {last}\nSWAP {conveyor} {last}\nM {last}\nX {toggle}\nM {toggle}\n}}\nM 0 1\n",
            prefix = inner.prefix,
            body = inner.body,
            conveyor = inner.next_qubit,
            last = inner.next_qubit + 1,
        );
        let paths = ReferencePaths::compute(&parse(&text));
        paths.assert_agree(&text);
        // The inner repeat folds in the first outer iteration, before the outer repeat holds
        // the snapshot. The outer repeat then finds its period-2 cycle after 4 iterations and
        // copies 40 of the 41 later ones; the inner repeat folds again in the leftover one.
        assert_eq!(paths.folded_stats.folded_repeats, 3, "{text}");
        assert_eq!(paths.folded_stats.longest_period, 3, "{text}");
        assert!(paths.folded_stats.reused_iterations >= 30, "{text}");
    });
}

#[test]
fn loops_that_do_not_recur_within_the_probe_bound_are_iterated() {
    with_forced_probes(|| {
        // A 70-qubit rotor has period 70, beyond the iterations a probe may run.
        let registers = CycleRegisters::new(0, 70);
        let text = registers.circuit(300);
        let paths = ReferencePaths::compute(&parse(&text));
        paths.assert_agree(&text);
        assert_eq!(paths.folded_stats, ReferenceExecutionStats::default());
    });
}

#[test]
fn fold_probes_are_skipped_when_the_saved_iterations_cannot_repay_them() {
    // Without forced probes, a two-iteration memory experiment saves at most one round, which
    // costs less than a snapshot and comparison, while a long one folds.
    let probability = Probability::try_new(0.001).expect("valid generated-circuit noise");
    for (rounds, folds) in [(3, false), (200, true)] {
        let params = CodeCircuitParams::try_new(
            CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX),
            CodeDistance::try_new(9).expect("valid code distance"),
            RoundCount::try_new(rounds).expect("valid round count"),
        )
        .expect("valid surface-code parameters")
        .with_after_clifford_depolarization(probability);
        let generated = params.generate().expect("generate surface circuit");
        let paths = ReferencePaths::compute(generated.circuit());
        paths.assert_agree(&format!("{rounds} rounds"));
        assert_eq!(
            paths.folded_stats.folded_repeats == 1,
            folds,
            "{rounds} rounds"
        );
    }
}
