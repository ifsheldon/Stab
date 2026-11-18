#![allow(
    clippy::expect_used,
    reason = "hostile-nesting regressions use direct fixture assertions"
)]

//! Models built through the public API may nest repeat blocks far deeper than the parser's
//! 256-level limit. Every whole-model operation must then either finish without recursing per
//! nesting level or refuse the model before recursing, so a thread with a small stack survives.

use std::ops::ControlFlow;

use stab_model::{
    Cancellation, Circuit, DemDetectorId, DemErrorMechanismTraversalLimits, DemErrorMechanismView,
    DemErrorMechanismVisitor, DemInstruction, DemRepeatBlock, DemRepeatCount, DemTarget,
    DetectorErrorModel, FoldedCircuitSummary, FoldedWalkError, Probability, RepeatBlock,
    RepeatCount, RepeatNestingLimit,
};

/// Far deeper than any parsed model, so per-level recursion would need tens of MiB of stack.
const DEEP: usize = 200_000;
/// Printed text grows quadratically with depth through indentation, so printing is checked at a
/// depth that still overflows a recursive printer on the small stack.
const PRINT_DEPTH: usize = 2_000;
const SMALL_STACK: usize = 64 * 1024;

fn on_small_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(work)
        .expect("spawn small-stack worker")
        .join()
        .expect("deep model operations stay within a small stack");
}

const LEAF: &str = "QUBIT_COORDS(1, 2) 3\nSHIFT_COORDS(0, 1)\nM 3\nDETECTOR(4, 5) rec[-1]\n";

/// `depth` tagged repeat blocks around [`LEAF`]; the innermost `doubling` repeat twice and the
/// rest once.
fn nested_circuit(depth: usize, doubling: usize) -> Circuit {
    let mut circuit = Circuit::from_stim_str(LEAF).expect("leaf circuit");
    for level in 0..depth {
        let mut outer = Circuit::new();
        outer.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(if level < doubling { 2 } else { 1 }).expect("repeat count"),
            circuit,
            Some("t".to_owned()),
        ));
        circuit = outer;
    }
    circuit
}

/// Canonical text of [`nested_circuit`], written independently of the printer.
fn nested_circuit_text(depth: usize, doubling: usize) -> String {
    let mut text = String::new();
    for level in 0..depth {
        text.push_str(&" ".repeat(4 * level));
        let count = if depth - level <= doubling { 2 } else { 1 };
        text.push_str(&format!("REPEAT[t] {count} {{\n"));
    }
    for line in LEAF.lines() {
        text.push_str(&" ".repeat(4 * depth));
        text.push_str(line);
        text.push('\n');
    }
    for level in (0..depth).rev() {
        text.push_str(&" ".repeat(4 * level));
        text.push_str("}\n");
    }
    text
}

#[test]
fn deeply_nested_circuit_counts_fingerprints_clones_compares_and_drops_on_a_small_stack() {
    on_small_stack(|| {
        let circuit = nested_circuit(DEEP, DEEP);
        let cloned = circuit.clone();
        assert_eq!(cloned, circuit);
        assert_eq!(cloned.fingerprint(), circuit.fingerprint());
        assert_ne!(nested_circuit(DEEP, DEEP - 1), circuit);
        assert_ne!(
            nested_circuit(DEEP, DEEP - 1).fingerprint(),
            circuit.fingerprint()
        );

        // 2^200000 executions overflow every expanded count.
        assert!(circuit.count_measurements().is_err());
        assert!(circuit.count_detectors().is_err());
        assert_eq!(circuit.count_ticks().expect("no ticks"), 0);
        assert_eq!(circuit.count_qubits(), 4);
        assert_eq!(circuit.count_observables().expect("no observables"), 0);
        assert_eq!(circuit.count_sweep_bits().expect("no sweep bits"), 0);
        // Doubling the shift 200000 times overflows to infinity, which is refused.
        assert!(circuit.final_coordinate_shift().is_err());
        assert!(circuit.admitted_bytes() > 0);

        let walked = FoldedCircuitSummary::of(
            &circuit,
            RepeatNestingLimit::try_new(RepeatNestingLimit::HARD_MAX).expect("limit"),
            &Cancellation::never(),
        );
        assert_eq!(
            walked,
            Err(FoldedWalkError::RepeatNesting {
                depth: RepeatNestingLimit::HARD_MAX + 1,
                limit: RepeatNestingLimit::HARD_MAX,
            })
        );

        drop(cloned);
        drop(circuit);
    });
}

#[test]
fn deeply_nested_circuit_prints_on_a_small_stack() {
    on_small_stack(|| {
        let circuit = nested_circuit(PRINT_DEPTH, 3);
        let expected = nested_circuit_text(PRINT_DEPTH, 3);
        assert_eq!(circuit.to_stim_string(), expected);
        assert_eq!(circuit.to_stim_bytes(), expected.into_bytes());
    });
}

#[test]
fn deeply_nested_circuit_finds_coordinates_on_a_small_stack() {
    // Each repeat body's detector count and coordinate shift is computed once per query, so a
    // query that recomputed them at every enclosing level would take time quadratic in depth
    // and not finish at this depth.
    on_small_stack(|| {
        let circuit = nested_circuit(DEEP, 10);
        assert_eq!(
            circuit.final_coordinate_shift().expect("coordinate shift"),
            [0.0, 1024.0]
        );
        let qubits = circuit
            .final_qubit_coordinates()
            .expect("qubit coordinates");
        assert_eq!(qubits.values().next(), Some(&vec![1.0, 2.0 + 1023.0]));

        let detectors = circuit
            .detector_coordinates_for([
                stab_model::CircuitDetectorId::new(3),
                stab_model::CircuitDetectorId::new(1000),
            ])
            .expect("detector coordinates");
        assert_eq!(
            detectors.into_values().collect::<Vec<_>>(),
            [vec![4.0, 5.0 + 4.0], vec![4.0, 5.0 + 1001.0]]
        );
        assert_eq!(circuit.count_detectors().expect("detectors"), 1024);
    });
}

fn nested_dem(depth: usize) -> DetectorErrorModel {
    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.125).expect("probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            None,
        )
        .expect("error instruction"),
    );
    for _ in 0..depth {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), model, None));
        model = outer;
    }
    model
}

#[test]
fn deeply_nested_dem_prints_fingerprints_and_visits_on_a_small_stack() {
    on_small_stack(|| {
        let deep = nested_dem(DEEP);
        assert_eq!(deep.clone(), deep);
        assert_eq!(deep.count_detectors().expect("detectors"), 1);
        assert_eq!(deep.fingerprint(), deep.clone().fingerprint());
        assert_eq!(
            deep.final_coordinate_shift()
                .expect("empty coordinate shift"),
            Vec::<f64>::new()
        );
        // The folded visit keeps its open repeat bodies on the heap, so it accepts any depth.
        assert_eq!(
            deep.detector_coordinates().expect("detector coordinates"),
            [(DemDetectorId::try_new(0).expect("D0"), Vec::new())].into()
        );
        let mut mechanisms = Vec::new();
        assert_eq!(
            deep.try_visit_error_mechanisms(
                DemErrorMechanismTraversalLimits::new(1, 1),
                &mut CollectMechanisms(&mut mechanisms),
            )
            .expect("error mechanisms"),
            ControlFlow::Continue(())
        );
        assert_eq!(mechanisms, [(0.125, 1)]);

        let printed = nested_dem(PRINT_DEPTH);
        let mut expected = String::new();
        for level in 0..PRINT_DEPTH {
            expected.push_str(&" ".repeat(4 * level));
            expected.push_str("repeat 1 {\n");
        }
        expected.push_str(&" ".repeat(4 * PRINT_DEPTH));
        expected.push_str("error(0.125) D0\n");
        for level in (0..PRINT_DEPTH).rev() {
            expected.push_str(&" ".repeat(4 * level));
            expected.push_str("}\n");
        }
        assert_eq!(printed.to_dem_string(), expected);
        assert_eq!(printed.to_dem_bytes(), expected.into_bytes());
    });
}

struct CollectMechanisms<'a>(&'a mut Vec<(f64, usize)>);

impl DemErrorMechanismVisitor for CollectMechanisms<'_> {
    type Error = std::convert::Infallible;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        self.0
            .push((mechanism.probability().get(), mechanism.targets().len()));
        Ok(ControlFlow::Continue(()))
    }
}

/// `depth` repeat-once blocks around a detector that shifts detector ids by `detector_shift`
/// and the second coordinate by one.
fn nested_coordinate_dem(depth: usize, detector_shift: u64) -> DetectorErrorModel {
    let mut model = DetectorErrorModel::from_dem_str(&format!(
        "shift_detectors(0, 1) {detector_shift}\ndetector(4, 5) D0\n"
    ))
    .expect("leaf model");
    for _ in 0..depth {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), model, None));
        model = outer;
    }
    model
}

#[test]
fn deeply_nested_dem_finds_coordinates_on_a_small_stack() {
    // A body without a detector shift is visited level by level, and a body with one is scanned
    // flattened; both keep their open bodies on the heap. Each body's coordinate shift is
    // computed once per query, so recomputing it at every enclosing level would take time
    // quadratic in depth and not finish at this depth.
    on_small_stack(|| {
        for (detector_shift, detector) in [(0, 0), (1, 1)] {
            let model = nested_coordinate_dem(DEEP, detector_shift);
            let detector = DemDetectorId::try_new(detector).expect("detector id");
            assert_eq!(
                model
                    .coordinates_of_detector(detector)
                    .expect("detector coordinates"),
                [4.0, 6.0]
            );
            assert_eq!(
                model.final_coordinate_shift().expect("coordinate shift"),
                [0.0, 1.0]
            );
        }
    });
}

#[test]
fn maximally_nested_dem_text_parses_and_round_trips_on_a_small_stack() {
    // The parser keeps one heap frame per open repeat block, so text at the parse nesting
    // limit, which a recursive parser needs far more than the small stack for in a debug
    // build, parses there too.
    on_small_stack(|| {
        let depth = RepeatNestingLimit::HARD_MAX;
        let model = nested_dem(depth);
        let text = model.to_dem_string();
        let parsed = DetectorErrorModel::from_dem_str(&text).expect("maximally nested text");
        assert_eq!(parsed, model);
        assert_eq!(parsed.to_dem_string(), text);

        let too_deep = format!("repeat 1 {{\n{text}}}\n");
        let refused = DetectorErrorModel::from_dem_str(&too_deep).expect_err("one level too deep");
        assert!(
            refused
                .to_string()
                .contains("repeat nesting exceeds current limit 256"),
            "{refused}"
        );
    });
}
