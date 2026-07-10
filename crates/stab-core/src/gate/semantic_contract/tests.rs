#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "contract tests use direct assertions for impossible canonical metadata states"
)]

use std::collections::{BTreeMap, BTreeSet};

use super::{
    GateSemanticFamily, GateShapeExclusion, GateSurface, GateSurfaceBehavior, GateTargetPattern,
};
use crate::{Circuit, CompiledSampler, Gate, MeasureRecordOffset, Pauli, QubitId, Target};

#[test]
fn gate_surface_contract_covers_every_canonical_gate_surface_and_target_pattern() {
    let mut behavior_counts = BTreeMap::new();
    let mut gate_count = 0;

    for gate in Gate::all() {
        gate_count += 1;
        let contract = gate.surface_contract();
        assert_eq!(contract.gate(), gate);
        assert!(!contract.target_patterns().is_empty());
        assert_eq!(
            contract
                .target_patterns()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            contract.target_patterns().len(),
            "{} has duplicate target patterns",
            gate.canonical_name()
        );

        for surface in GateSurface::ALL {
            for &pattern in contract.target_patterns() {
                let decision = contract
                    .decision(surface, pattern)
                    .expect("declared target pattern must have one decision");
                if decision.behavior == GateSurfaceBehavior::UnsupportedShape {
                    assert!(
                        decision.exclusion.is_some(),
                        "{} {surface:?} {pattern:?} lacks an exclusion",
                        gate.canonical_name()
                    );
                } else {
                    assert_eq!(
                        decision.exclusion,
                        None,
                        "{} {surface:?} {pattern:?} has a stray exclusion",
                        gate.canonical_name()
                    );
                }
                *behavior_counts.entry(decision.behavior).or_insert(0usize) += 1;
            }
        }
    }

    assert_eq!(gate_count, 81);
    assert_eq!(
        behavior_counts.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            GateSurfaceBehavior::Execute,
            GateSurfaceBehavior::SemanticNoop,
            GateSurfaceBehavior::Annotation,
            GateSurfaceBehavior::LowerThenExecute,
            GateSurfaceBehavior::UnsupportedShape,
            GateSurfaceBehavior::NotApplicable,
        ])
    );
}

#[test]
fn gate_surface_contract_target_patterns_are_accepted_and_classified() {
    for gate in Gate::all() {
        let args = representative_args(gate);
        let contract = gate.surface_contract();
        for &pattern in contract.target_patterns() {
            let targets = representative_targets(pattern);
            gate.validate(&args, &targets).unwrap_or_else(|error| {
                panic!(
                    "{} pattern {pattern:?} must satisfy its canonical parser rule: {error}",
                    gate.canonical_name()
                )
            });
            let classified = contract
                .classify_target_groups(&targets)
                .expect("parser-accepted targets must be classified");
            assert!(
                classified.contains(&pattern),
                "{} targets for {pattern:?} classified as {classified:?}",
                gate.canonical_name()
            );
        }
    }
}

#[test]
fn gate_surface_contract_unsupported_shapes_have_narrow_typed_reasons() {
    for gate in Gate::all() {
        let contract = gate.surface_contract();
        for surface in GateSurface::ALL {
            for &pattern in contract.target_patterns() {
                let decision = contract
                    .decision(surface, pattern)
                    .expect("declared target pattern");
                let Some(exclusion) = decision.exclusion else {
                    continue;
                };
                match exclusion {
                    GateShapeExclusion::AntiHermitianPauliProduct => {
                        assert_eq!(pattern, GateTargetPattern::AntiHermitianPauliProduct);
                        assert!(matches!(gate.canonical_name(), "MPP" | "SPP" | "SPP_DAG"));
                    }
                    GateShapeExclusion::QuantumCannotControlClassical => {
                        assert!(matches!(
                            pattern,
                            GateTargetPattern::QubitRecord
                                | GateTargetPattern::QubitSweep
                                | GateTargetPattern::RecordQubit
                                | GateTargetPattern::SweepQubit
                        ));
                    }
                    GateShapeExclusion::ClassicalOnlyPairRequiresSymmetricCz => {
                        assert_ne!(gate.canonical_name(), "CZ");
                        assert!(matches!(
                            pattern,
                            GateTargetPattern::RecordRecord
                                | GateTargetPattern::RecordSweep
                                | GateTargetPattern::SweepRecord
                                | GateTargetPattern::SweepSweep
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn gate_surface_contract_classical_control_matrix_is_directional() {
    let cx = Gate::from_name("CX").expect("CX").surface_contract();
    let xcz = Gate::from_name("XCZ").expect("XCZ").surface_contract();
    let cz = Gate::from_name("CZ").expect("CZ").surface_contract();

    assert_behavior(
        cx,
        GateSurface::ReferenceSampler,
        GateTargetPattern::RecordQubit,
        GateSurfaceBehavior::Execute,
    );
    assert_unsupported(
        cx,
        GateSurface::ReferenceSampler,
        GateTargetPattern::QubitRecord,
        GateShapeExclusion::QuantumCannotControlClassical,
    );
    assert_behavior(
        xcz,
        GateSurface::ReferenceSampler,
        GateTargetPattern::QubitRecord,
        GateSurfaceBehavior::Execute,
    );
    assert_unsupported(
        xcz,
        GateSurface::ReferenceSampler,
        GateTargetPattern::RecordQubit,
        GateShapeExclusion::QuantumCannotControlClassical,
    );
    assert_behavior(
        cz,
        GateSurface::ReferenceSampler,
        GateTargetPattern::RecordRecord,
        GateSurfaceBehavior::SemanticNoop,
    );
    assert_behavior(
        cz,
        GateSurface::DetectionConverter,
        GateTargetPattern::QubitSweep,
        GateSurfaceBehavior::Execute,
    );
    assert_behavior(
        cz,
        GateSurface::ErrorAnalyzer,
        GateTargetPattern::QubitSweep,
        GateSurfaceBehavior::SemanticNoop,
    );
    assert_behavior(
        cz,
        GateSurface::MeasurementSampler,
        GateTargetPattern::SweepQubit,
        GateSurfaceBehavior::SemanticNoop,
    );

    for gate_name in ["CX", "CY", "CZ", "XCZ", "YCZ"] {
        let contract = Gate::from_name(gate_name)
            .expect(gate_name)
            .surface_contract();
        for pattern in [
            GateTargetPattern::RecordQubit,
            GateTargetPattern::QubitRecord,
        ] {
            assert_behavior(
                contract,
                GateSurface::FlowGenerator,
                pattern,
                GateSurfaceBehavior::Execute,
            );
        }
        for pattern in [GateTargetPattern::SweepQubit, GateTargetPattern::QubitSweep] {
            assert_behavior(
                contract,
                GateSurface::FlowGenerator,
                pattern,
                GateSurfaceBehavior::SemanticNoop,
            );
        }
        for pattern in [
            GateTargetPattern::RecordRecord,
            GateTargetPattern::RecordSweep,
            GateTargetPattern::SweepRecord,
            GateTargetPattern::SweepSweep,
        ] {
            assert_behavior(
                contract,
                GateSurface::FlowGenerator,
                pattern,
                GateSurfaceBehavior::SemanticNoop,
            );
        }
    }
}

#[test]
fn gate_surface_contract_classifies_mixed_classical_control_groups_in_order() {
    let contract = Gate::from_name("CZ").expect("CZ").surface_contract();
    let q0 = || Target::qubit(QubitId::new(0).expect("q0"), false);
    let q1 = || Target::qubit(QubitId::new(1).expect("q1"), false);
    let rec1 =
        || Target::measurement_record(MeasureRecordOffset::try_new(-1).expect("record offset"));
    let rec2 =
        || Target::measurement_record(MeasureRecordOffset::try_new(-2).expect("record offset"));
    let sweep0 = || Target::sweep_bit(0);
    let sweep1 = || Target::sweep_bit(1);
    let targets = vec![
        q0(),
        q1(),
        rec1(),
        q0(),
        sweep0(),
        q0(),
        q0(),
        rec1(),
        q0(),
        sweep0(),
        rec1(),
        rec2(),
        rec1(),
        sweep0(),
        sweep0(),
        rec1(),
        sweep0(),
        sweep1(),
        rec1(),
        q1(),
    ];

    assert_eq!(
        contract
            .classify_target_groups(&targets)
            .expect("mixed CZ target groups"),
        vec![
            GateTargetPattern::QubitQubit,
            GateTargetPattern::RecordQubit,
            GateTargetPattern::SweepQubit,
            GateTargetPattern::QubitRecord,
            GateTargetPattern::QubitSweep,
            GateTargetPattern::RecordRecord,
            GateTargetPattern::RecordSweep,
            GateTargetPattern::SweepRecord,
            GateTargetPattern::SweepSweep,
        ]
    );
}

#[test]
fn gate_surface_contract_classifies_hermitian_and_anti_hermitian_products() {
    let mpp = Gate::from_name("MPP").expect("MPP").surface_contract();
    let q0 = QubitId::new(0).expect("q0");
    let q1 = QubitId::new(1).expect("q1");
    let targets = vec![
        Target::pauli(Pauli::X, q0, false),
        Target::combiner(),
        Target::pauli(Pauli::Z, q0, false),
        Target::pauli(Pauli::Y, q1, false),
    ];

    assert_eq!(
        mpp.classify_target_groups(&targets)
            .expect("valid mixed MPP products"),
        vec![
            GateTargetPattern::AntiHermitianPauliProduct,
            GateTargetPattern::HermitianPauliProduct,
        ]
    );
    for surface in GateSurface::ALL {
        if surface == GateSurface::Parser {
            assert_behavior(
                mpp,
                surface,
                GateTargetPattern::AntiHermitianPauliProduct,
                GateSurfaceBehavior::Execute,
            );
        } else {
            assert_unsupported(
                mpp,
                surface,
                GateTargetPattern::AntiHermitianPauliProduct,
                GateShapeExclusion::AntiHermitianPauliProduct,
            );
        }
    }
}

#[test]
fn gate_surface_contract_pauli_product_phase_matches_sampler_validation() {
    let mpp = Gate::from_name("MPP").expect("MPP").surface_contract();
    let choices = [
        (Pauli::X, 0_u32),
        (Pauli::Y, 0),
        (Pauli::Z, 0),
        (Pauli::X, 1),
        (Pauli::Y, 1),
        (Pauli::Z, 1),
    ];

    for factor_count in 1..=4_u32 {
        for mut encoded in 0..choices.len().pow(factor_count) {
            let mut targets = Vec::with_capacity(factor_count as usize * 2 - 1);
            let mut terms = Vec::with_capacity(factor_count as usize);
            for factor_index in 0..factor_count {
                let (pauli, qubit) = choices
                    .get(encoded % choices.len())
                    .copied()
                    .expect("base-6 digit must select one Pauli target");
                encoded /= choices.len();
                if factor_index > 0 {
                    targets.push(Target::combiner());
                }
                targets.push(Target::pauli(
                    pauli,
                    QubitId::new(qubit).expect("small qubit id"),
                    false,
                ));
                terms.push(format!("{pauli}{qubit}"));
            }

            let pattern = mpp
                .classify_target_groups(&targets)
                .expect("valid generated MPP product")
                .into_iter()
                .next()
                .expect("one generated product pattern");
            let circuit = Circuit::from_stim_str(&format!("MPP {}\n", terms.join("*")))
                .expect("generated MPP parses");
            let compilation = CompiledSampler::compile(&circuit);
            match pattern {
                GateTargetPattern::HermitianPauliProduct => {
                    compilation.expect("Hermitian product must compile");
                }
                GateTargetPattern::AntiHermitianPauliProduct => {
                    let error = compilation.expect_err("anti-Hermitian product must reject");
                    assert!(error.to_string().contains("anti-Hermitian"), "{error}");
                }
                other => panic!("generated Pauli product classified as {other:?}"),
            }
        }
    }
}

#[test]
fn gate_surface_contract_preserves_empty_correlated_error_branch_state() {
    let correlated_error = Gate::from_name("E")
        .expect("correlated error")
        .surface_contract();
    for surface in [
        GateSurface::MeasurementSampler,
        GateSurface::DetectorFrame,
        GateSurface::DetectionSampler,
        GateSurface::ErrorAnalyzer,
    ] {
        assert_behavior(
            correlated_error,
            surface,
            GateTargetPattern::EmptyTargetList,
            GateSurfaceBehavior::Execute,
        );
    }
    for surface in [
        GateSurface::ReferenceSampler,
        GateSurface::DetectionConverter,
        GateSurface::FlowGenerator,
    ] {
        assert_behavior(
            correlated_error,
            surface,
            GateTargetPattern::EmptyTargetList,
            GateSurfaceBehavior::SemanticNoop,
        );
    }
}

#[test]
fn gate_surface_contract_family_assignments_are_canonical_metadata() {
    let expected = [
        ("MPAD", GateSemanticFamily::MeasurementPad),
        ("M", GateSemanticFamily::Measurement),
        ("MR", GateSemanticFamily::MeasureReset),
        ("R", GateSemanticFamily::Reset),
        ("CX", GateSemanticFamily::ForwardClassicalControl),
        ("CZ", GateSemanticFamily::SymmetricClassicalControl),
        ("XCZ", GateSemanticFamily::ReverseClassicalControl),
        ("DEPOLARIZE1", GateSemanticFamily::Depolarization),
        ("X_ERROR", GateSemanticFamily::PauliNoise),
        ("I_ERROR", GateSemanticFamily::IdentityNoise),
        ("PAULI_CHANNEL_1", GateSemanticFamily::PauliChannel),
        ("E", GateSemanticFamily::CorrelatedError),
        ("HERALDED_ERASE", GateSemanticFamily::HeraldedNoise),
        ("MPP", GateSemanticFamily::PauliProductMeasurement),
        ("SPP", GateSemanticFamily::PauliProductPhase),
        ("MXX", GateSemanticFamily::PairMeasurement),
        ("H", GateSemanticFamily::FixedTableau),
        ("DETECTOR", GateSemanticFamily::Annotation),
        ("REPEAT", GateSemanticFamily::ControlFlow),
    ];

    for (name, family) in expected {
        assert_eq!(
            Gate::from_name(name).expect(name).info.semantic_family,
            family,
            "{name} semantic family"
        );
    }

    assert_eq!(
        Gate::all()
            .map(|gate| gate.info.semantic_family)
            .collect::<BTreeSet<_>>(),
        GateSemanticFamily::ALL.into_iter().collect::<BTreeSet<_>>()
    );
    assert_eq!(
        GateSemanticFamily::ALL.map(GateSemanticFamily::as_str),
        GateSemanticFamily::NAMES
    );
    assert_eq!(
        GateSurface::ALL.map(GateSurface::as_str),
        GateSurface::NAMES
    );
}

#[test]
fn gate_surface_contract_annotation_and_constant_roles_are_explicit() {
    assert_eq!(
        Gate::from_name("DETECTOR")
            .expect("DETECTOR")
            .surface_contract()
            .target_patterns(),
        &[
            GateTargetPattern::EmptyTargetList,
            GateTargetPattern::DetectorDeclaration,
        ]
    );
    assert_eq!(
        Gate::from_name("OBSERVABLE_INCLUDE")
            .expect("OBSERVABLE_INCLUDE")
            .surface_contract()
            .target_patterns(),
        &[
            GateTargetPattern::EmptyTargetList,
            GateTargetPattern::ObservableDeclaration,
        ]
    );
    assert_eq!(
        Gate::from_name("MPAD")
            .expect("MPAD")
            .surface_contract()
            .target_patterns(),
        &[
            GateTargetPattern::EmptyTargetList,
            GateTargetPattern::MeasurementPad,
        ]
    );
    assert_behavior(
        Gate::from_name("M").expect("M").surface_contract(),
        GateSurface::MeasurementSampler,
        GateTargetPattern::EmptyTargetList,
        GateSurfaceBehavior::SemanticNoop,
    );
}

fn assert_behavior(
    contract: super::GateSurfaceContract,
    surface: GateSurface,
    pattern: GateTargetPattern,
    expected: GateSurfaceBehavior,
) {
    let decision = contract
        .decision(surface, pattern)
        .expect("target decision");
    assert_eq!(decision.behavior, expected);
    assert_eq!(decision.exclusion, None);
}

fn assert_unsupported(
    contract: super::GateSurfaceContract,
    surface: GateSurface,
    pattern: GateTargetPattern,
    exclusion: GateShapeExclusion,
) {
    let decision = contract
        .decision(surface, pattern)
        .expect("target decision");
    assert_eq!(decision.behavior, GateSurfaceBehavior::UnsupportedShape);
    assert_eq!(decision.exclusion, Some(exclusion));
}

fn representative_args(gate: Gate) -> Vec<f64> {
    match gate.arg_rule() {
        super::super::ArgRule::Exact(count) => vec![0.0; count],
        super::super::ArgRule::Any => Vec::new(),
        super::super::ArgRule::ZeroOrOneProbability => vec![0.125],
        super::super::ArgRule::ProbabilityList(count) => vec![0.0; count],
        super::super::ArgRule::AnyProbabilityList => Vec::new(),
        super::super::ArgRule::UnsignedInteger => vec![0.0],
    }
}

fn representative_targets(pattern: GateTargetPattern) -> Vec<Target> {
    let q0 = || Target::qubit(QubitId::new(0).expect("q0"), false);
    let q1 = || Target::qubit(QubitId::new(1).expect("q1"), false);
    let inverted_q0 = || Target::qubit(QubitId::new(0).expect("q0"), true);
    let rec1 =
        || Target::measurement_record(MeasureRecordOffset::try_new(-1).expect("record offset"));
    let rec2 =
        || Target::measurement_record(MeasureRecordOffset::try_new(-2).expect("record offset"));
    let sweep0 = || Target::sweep_bit(0);
    let sweep1 = || Target::sweep_bit(1);
    let x0 = || Target::pauli(Pauli::X, QubitId::new(0).expect("q0"), false);
    let z0 = || Target::pauli(Pauli::Z, QubitId::new(0).expect("q0"), false);
    let y1 = || Target::pauli(Pauli::Y, QubitId::new(1).expect("q1"), false);

    match pattern {
        GateTargetPattern::NoTargets | GateTargetPattern::EmptyTargetList => vec![],
        GateTargetPattern::PlainQubit | GateTargetPattern::QubitCoordinates => vec![q0()],
        GateTargetPattern::MeasurementQubit => vec![inverted_q0()],
        GateTargetPattern::MeasurementPad => vec![q0(), q1()],
        GateTargetPattern::PlainQubitPair | GateTargetPattern::QubitQubit => vec![q0(), q1()],
        GateTargetPattern::MeasurementQubitPair => vec![inverted_q0(), q1()],
        GateTargetPattern::DetectorDeclaration => vec![rec1()],
        GateTargetPattern::ObservableDeclaration => vec![rec1(), x0()],
        GateTargetPattern::HermitianPauliProduct => vec![x0(), Target::combiner(), y1()],
        GateTargetPattern::AntiHermitianPauliProduct => vec![x0(), Target::combiner(), z0()],
        GateTargetPattern::PauliList => vec![x0(), y1()],
        GateTargetPattern::RecordQubit => vec![rec1(), q0()],
        GateTargetPattern::SweepQubit => vec![sweep0(), q0()],
        GateTargetPattern::QubitRecord => vec![q0(), rec1()],
        GateTargetPattern::QubitSweep => vec![q0(), sweep0()],
        GateTargetPattern::RecordRecord => vec![rec1(), rec2()],
        GateTargetPattern::RecordSweep => vec![rec1(), sweep0()],
        GateTargetPattern::SweepRecord => vec![sweep0(), rec1()],
        GateTargetPattern::SweepSweep => vec![sweep0(), sweep1()],
    }
}
