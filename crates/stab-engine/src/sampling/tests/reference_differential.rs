//! Reference-sample differentials across the three engine paths.
//!
//! The iterated general tableau is the semantic baseline. The folded general path (which reuses
//! one loop iteration when the state snapshot repeats) and the CSS fast path (which only accepts
//! H/CX/R/M/MR-style programs) must reproduce it exactly, including on loops whose iterations
//! change measurement signs, cycle with a period longer than one, or act on entangled state.

use rand::RngExt as _;

use super::*;

struct ReferencePaths {
    iterated: Vec<bool>,
    folded: Vec<bool>,
    folded_stats: ReferenceExecutionStats,
    css: Option<Vec<bool>>,
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
        let iterated =
            super::super::api::compute_reference_sample_without_fast_path(&iterated_plan.inner)
                .expect("iterated general reference");
        let (folded, folded_stats) = reference_with_stats(&folded_plan);
        assert_eq!(
            super::super::api::compute_reference_sample_without_fast_path(&folded_plan.inner)
                .expect("product-policy general reference"),
            folded,
            "the product snapshot decision must not change the folded result"
        );
        let inner = &folded_plan.inner;
        let mut record = Vec::with_capacity(inner.measurement_count);
        let mut css = Vec::with_capacity(inner.measurement_count);
        let css_accepted = super::super::reference_tableau::try_compute_reference_sample(
            &inner.operations,
            inner.qubit_count,
            inner.measurement_count,
            inner.expanded_operation_count,
            &mut record,
            &mut css,
        )
        .expect("execute CSS reference tableau");
        assert_eq!(
            folded_plan
                .try_reference_sample()
                .expect("product reference sample"),
            iterated,
            "product reference sample"
        );
        Self {
            iterated,
            folded,
            folded_stats,
            css: css_accepted.then_some(css),
        }
    }

    fn assert_agree(&self, label: &str) {
        assert_eq!(self.folded, self.iterated, "folded vs iterated: {label}");
        if let Some(css) = &self.css {
            assert_eq!(css, &self.iterated, "CSS fast path vs iterated: {label}");
        }
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
fn folded_and_css_references_match_iteration_on_sign_changing_periodic_and_entangled_loops() {
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
        assert_eq!(paths.iterated, expected, "iterated baseline: {label}");
        paths.assert_agree(label);
        if must_fold {
            assert_eq!(
                paths.folded_stats.folded_repeats, 1,
                "{label} must exercise the fold path"
            );
        }
        let uses_non_css_pauli = text.lines().any(|line| line.trim_start().starts_with("X "));
        if !uses_non_css_pauli {
            assert!(paths.css.is_some(), "{label} is a CSS program");
        }
    }
}

#[test]
fn folded_reference_matches_iteration_on_surface_code_with_inserted_paulis() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(25).expect("valid round count"),
        CodeDistance::try_new(3).expect("valid code distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("valid surface-code parameters");
    let generated = generate_surface_code_circuit(&params).expect("generate surface circuit");
    let text = generated.circuit().to_stim_string();
    let baseline = ReferencePaths::compute(generated.circuit());
    baseline.assert_agree("surface code");
    assert!(
        baseline.css.is_some(),
        "the noiseless surface code is a CSS program"
    );

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
            paths.iterated, baseline.iterated,
            "{label}: the inserted Pauli must change reference signs"
        );
    }
}

#[test]
fn css_and_folded_references_match_iteration_on_seeded_random_css_circuits() {
    let mut rng = SmallRng::seed_from_u64(0xC55_2EF);
    for qubits in [1_usize, 2, 3, 5, 8, 17, 64, 65, 128, 300] {
        for _ in 0..3 {
            let mut text = String::new();
            let emit = |text: &mut String, rng: &mut SmallRng, operations: usize| {
                for _ in 0..operations {
                    let qubit = rng.random_range(0..qubits);
                    match rng.random_range(0..5) {
                        0 => writeln!(text, "H {qubit}"),
                        1 if qubits > 1 => {
                            let target = (qubit + rng.random_range(1..qubits)) % qubits;
                            writeln!(text, "CX {qubit} {target}")
                        }
                        1 | 2 => writeln!(text, "R {qubit}"),
                        3 => writeln!(text, "M {qubit}"),
                        _ => writeln!(text, "MR {qubit}"),
                    }
                    .expect("write generated instruction");
                }
            };
            emit(&mut text, &mut rng, 2 * qubits.min(64));
            let repetitions = if qubits <= 8 { 40 } else { 3 };
            text.push_str(&format!("REPEAT {repetitions} {{\n"));
            emit(&mut text, &mut rng, qubits.min(48));
            text.push_str("}\nM");
            for qubit in 0..qubits {
                write!(text, " {qubit}").expect("write final measurement");
            }
            text.push('\n');
            let paths = ReferencePaths::compute(&parse(&text));
            let label = format!("{qubits}-qubit random CSS circuit:\n{text}");
            assert!(paths.css.is_some(), "CSS fast path declined {label}");
            paths.assert_agree(&label);
        }
    }
}
