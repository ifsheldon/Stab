#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "CQ2 Algebra qualification uses compact exhaustive tables and direct contract assertions"
)]

use std::error::Error as _;
use std::str::FromStr;

use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_core::{
    BitError, CliffordString, FlexPauliString, Flow, FlowMeasurementIndex, Gate, PauliBasis,
    PauliPhase, PauliSign, PauliString, SingleQubitClifford, StabilizerError, StabilizerResource,
    StabilizerResult,
};

const STIM_ALL_CLIFFORDS_ORDER: [SingleQubitClifford; 24] = [
    SingleQubitClifford::I,
    SingleQubitClifford::X,
    SingleQubitClifford::Y,
    SingleQubitClifford::Z,
    SingleQubitClifford::Hxy,
    SingleQubitClifford::S,
    SingleQubitClifford::SDag,
    SingleQubitClifford::Hnxy,
    SingleQubitClifford::H,
    SingleQubitClifford::SqrtYDag,
    SingleQubitClifford::Hnxz,
    SingleQubitClifford::SqrtY,
    SingleQubitClifford::Hyz,
    SingleQubitClifford::Hnyz,
    SingleQubitClifford::SqrtX,
    SingleQubitClifford::SqrtXDag,
    SingleQubitClifford::Cxyz,
    SingleQubitClifford::Cxynz,
    SingleQubitClifford::Cnxyz,
    SingleQubitClifford::Cxnyz,
    SingleQubitClifford::Czyx,
    SingleQubitClifford::Cznyx,
    SingleQubitClifford::Cnzyx,
    SingleQubitClifford::Czynx,
];

#[test]
fn cq2_algebra_pauli_value_types_have_complete_scalar_contract() {
    for (sign, negative, text) in [(PauliSign::Plus, false, "+"), (PauliSign::Minus, true, "-")] {
        assert_eq!(sign.is_negative(), negative);
        assert_eq!(sign.to_string(), text);
    }

    for (phase, real, negative, sign, text) in [
        (PauliPhase::Plus, true, false, PauliSign::Plus, "+"),
        (PauliPhase::PlusI, false, false, PauliSign::Plus, "+i"),
        (PauliPhase::Minus, true, true, PauliSign::Minus, "-"),
        (PauliPhase::MinusI, false, true, PauliSign::Minus, "-i"),
    ] {
        assert_eq!(phase.is_real(), real);
        assert_eq!(phase.is_imaginary(), !real);
        assert_eq!(phase.is_negative(), negative);
        assert_eq!(phase.sign(), sign);
        assert_eq!(phase.to_string(), text);
    }

    let bases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    let expected_products = [
        [
            (PauliBasis::I, PauliPhase::Plus),
            (PauliBasis::X, PauliPhase::Plus),
            (PauliBasis::Y, PauliPhase::Plus),
            (PauliBasis::Z, PauliPhase::Plus),
        ],
        [
            (PauliBasis::X, PauliPhase::Plus),
            (PauliBasis::I, PauliPhase::Plus),
            (PauliBasis::Z, PauliPhase::PlusI),
            (PauliBasis::Y, PauliPhase::MinusI),
        ],
        [
            (PauliBasis::Y, PauliPhase::Plus),
            (PauliBasis::Z, PauliPhase::MinusI),
            (PauliBasis::I, PauliPhase::Plus),
            (PauliBasis::X, PauliPhase::PlusI),
        ],
        [
            (PauliBasis::Z, PauliPhase::Plus),
            (PauliBasis::Y, PauliPhase::PlusI),
            (PauliBasis::X, PauliPhase::MinusI),
            (PauliBasis::I, PauliPhase::Plus),
        ],
    ];
    for (left_index, left) in bases.into_iter().enumerate() {
        assert_eq!(
            PauliBasis::from_xz(left.x_bit(), left.z_bit()),
            left,
            "round trip {left:?}"
        );
        assert_eq!(left.to_string(), ["_", "X", "Y", "Z"][left_index]);
        for (right_index, right) in bases.into_iter().enumerate() {
            let expected = expected_products[left_index][right_index];
            assert_eq!(left.multiply(right), expected, "{left:?} * {right:?}");
            assert_eq!(
                left.log_i_scalar_byproduct(right),
                match expected.1 {
                    PauliPhase::Plus => 0,
                    PauliPhase::PlusI => 1,
                    PauliPhase::Minus => 2,
                    PauliPhase::MinusI => 3,
                }
            );
        }
    }
}

#[test]
fn cq2_algebra_pauli_owned_value_contract_is_typed_and_canonical() {
    let mut value = PauliString::from_bases(
        PauliSign::Minus,
        [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z],
    )
    .expect("construct Pauli");
    assert_eq!(value.to_string(), "-_XYZ");
    assert_eq!(value.sparse_string(), "-X1*Y2*Z3");
    assert_eq!(value.len(), 4);
    assert!(!value.is_empty());
    assert_eq!(value.sign(), PauliSign::Minus);
    assert_eq!(value.phase(), PauliPhase::Minus);
    assert_eq!(value.x_bits(), &[0b0110]);
    assert_eq!(value.z_bits(), &[0b1100]);
    assert_eq!(value.weight(), 3);
    assert!(!value.has_no_pauli_terms());
    assert_eq!(value.get(0), Some(PauliBasis::I));
    assert_eq!(value.get(1), Some(PauliBasis::X));
    assert_eq!(value.get(2), Some(PauliBasis::Y));
    assert_eq!(value.get(3), Some(PauliBasis::Z));
    assert_eq!(value.get(4), None);
    assert_eq!(
        value.active_terms().collect::<Vec<_>>(),
        vec![(1, PauliBasis::X), (2, PauliBasis::Y), (3, PauliBasis::Z)]
    );

    value.set(1, PauliBasis::I).expect("clear X");
    value.set(2, PauliBasis::I).expect("clear Y");
    value.set(3, PauliBasis::I).expect("clear Z");
    assert!(value.has_no_pauli_terms());
    assert_eq!(value.to_string(), "-____");
    value.set(0, PauliBasis::Y).expect("set Y");
    assert_eq!(value.to_string(), "-Y___");

    let before_error = value.clone();
    assert_eq!(
        value.set(4, PauliBasis::X),
        Err(StabilizerError::Bit(BitError::BitIndexOutOfRange {
            index: 4,
            len: 4,
        }))
    );
    assert_eq!(value, before_error);

    let empty = PauliString::identity(0).expect("empty identity");
    assert!(empty.is_empty());
    assert_eq!(empty.to_string(), "+");
    assert_eq!(
        PauliString::identity(5).expect("identity").to_string(),
        "+_____"
    );
    assert_eq!(
        PauliString::from_str("+IXYZ"),
        PauliString::from_str("+_XYZ")
    );
    assert_eq!(
        PauliString::from_str("X?"),
        Err(StabilizerError::InvalidPauliCharacter {
            character: '?',
            offset: 1,
        })
    );
}

#[test]
fn cq2_algebra_flex_pauli_contract_tracks_all_four_phases() {
    let bases = [PauliBasis::X, PauliBasis::I, PauliBasis::Z];
    for (phase, text) in [
        (PauliPhase::Plus, "+X_Z"),
        (PauliPhase::PlusI, "+iX_Z"),
        (PauliPhase::Minus, "-X_Z"),
        (PauliPhase::MinusI, "-iX_Z"),
    ] {
        let value =
            FlexPauliString::from_phase_and_bases(phase, bases).expect("construct flexible Pauli");
        assert_eq!(value.phase(), phase);
        assert_eq!(value.to_string(), text);
        assert_eq!(value.len(), 3);
        assert!(!value.is_empty());
        assert_eq!(value.get(0), Some(PauliBasis::X));
        assert_eq!(value.get(1), Some(PauliBasis::I));
        assert_eq!(value.get(2), Some(PauliBasis::Z));
        assert_eq!(value.get(3), None);
        assert_eq!(value.value().sign(), phase.sign());
    }

    let empty = FlexPauliString::identity(0).expect("empty flexible identity");
    assert!(empty.is_empty());
    assert_eq!(empty.to_string(), "+");
    assert_eq!(
        FlexPauliString::from_str("X8*Y2")
            .expect("sparse flexible Pauli")
            .to_string(),
        "+__Y_____X"
    );
    assert_eq!(
        FlexPauliString::from_str("X5*Y5")
            .expect("sparse phase accumulation")
            .to_string(),
        "+i_____Z"
    );
    assert_eq!(
        FlexPauliString::from_str("X")
            .expect("X")
            .multiply(&FlexPauliString::from_str("Y").expect("Y"))
            .expect("X times Y")
            .to_string(),
        "+iZ"
    );
    assert_eq!(
        FlexPauliString::from_str("-Z")
            .expect("real")
            .try_into_real()
            .expect("real conversion")
            .to_string(),
        "-Z"
    );
    assert_eq!(
        FlexPauliString::from_str("-iZ")
            .expect("imaginary")
            .try_into_real(),
        Err(StabilizerError::ImaginaryProduct {
            phase: PauliPhase::MinusI,
        })
    );
    assert_eq!(
        FlexPauliString::from_str("X*"),
        Err(StabilizerError::InvalidPauliCharacter {
            character: '*',
            offset: 1,
        })
    );
}

#[test]
fn cq2_algebra_single_qubit_clifford_contract_covers_values_and_names() {
    let all = SingleQubitClifford::all().collect::<Vec<_>>();
    assert_eq!(all.len(), 24);
    let mut canonical_names = std::collections::BTreeSet::new();
    let mut tokens = std::collections::BTreeSet::new();

    for value in all.iter().copied() {
        assert!(canonical_names.insert(value.canonical_name()));
        assert!(tokens.insert(value.token()));
        assert_eq!(value.to_string(), value.token());
        let gate = Gate::from_name(value.canonical_name()).expect("canonical gate");
        assert_eq!(SingleQubitClifford::from_gate(gate), Ok(value));
        assert_eq!(SingleQubitClifford::try_from(gate), Ok(value));
    }
    assert_eq!(canonical_names.len(), 24);
    assert_eq!(tokens.len(), 24);

    let cx = Gate::from_name("CX").expect("CX gate");
    assert_eq!(
        SingleQubitClifford::from_gate(cx),
        Err(StabilizerError::InvalidSingleQubitCliffordGate {
            gate: "CX".to_owned(),
        })
    );
}

#[test]
fn cq2_algebra_clifford_string_contract_covers_growth_and_composition() {
    let empty = CliffordString::identity(0).expect("empty Clifford string");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.to_string(), "");

    let mut value = CliffordString::from_gates([
        SingleQubitClifford::H,
        SingleQubitClifford::S,
        SingleQubitClifford::I,
    ])
    .expect("Clifford string");
    assert_eq!(value.len(), 3);
    assert!(!value.is_empty());
    assert_eq!(value.to_string(), "HI SI _I");
    assert_eq!(value.gate_at(0), Some(SingleQubitClifford::H));
    assert_eq!(value.gate_at(2), Some(SingleQubitClifford::I));
    assert_eq!(value.gate_at(3), None);
    assert_eq!(
        value.set_gate_at(3, SingleQubitClifford::X),
        Err(StabilizerError::CliffordIndexOutOfRange { index: 3, len: 3 })
    );
    assert_eq!(value.to_string(), "HI SI _I");
    value
        .set_gate_at(2, SingleQubitClifford::Z)
        .expect("set Clifford gate");
    assert_eq!(value.to_string(), "HI SI _Z");

    let suffix = CliffordString::from_gates([SingleQubitClifford::X]).expect("suffix");
    assert_eq!(
        value.concat(&suffix).expect("concat").to_string(),
        "HI SI _Z _X"
    );
    assert_eq!(suffix.repeat(3).expect("repeat").to_string(), "_X _X _X");
    assert_eq!(suffix.repeat(0).expect("zero repeat"), empty);

    let left = CliffordString::from_gates([SingleQubitClifford::H]).expect("left");
    let right = CliffordString::from_gates([SingleQubitClifford::H, SingleQubitClifford::S])
        .expect("right");
    let product = left.multiply(&right).expect("multiply with padding");
    let mut in_place = left.clone();
    in_place
        .right_multiply_in_place(&right)
        .expect("in-place multiply");
    assert_eq!(product, in_place);
    assert_eq!(product.to_string(), "_I SI");

    let identity_width = 552;
    let mut identity_left = CliffordString::from_gates(
        (0..identity_width).map(|index| STIM_ALL_CLIFFORDS_ORDER[index % 24]),
    )
    .expect("equal-width identity left operand");
    let identity_left_before = identity_left.clone();
    let identity_right = CliffordString::identity(identity_width).expect("identity right operand");
    let identity_right_before = identity_right.clone();
    identity_left
        .right_multiply_in_place(&identity_right)
        .expect("equal-width identity multiplication");
    assert_eq!(identity_left, identity_left_before);
    assert_eq!(identity_right, identity_right_before);

    let mut cycle_left = stab_core::stabilizers::CliffordString::from_gates(
        (0..identity_width).map(|index| STIM_ALL_CLIFFORDS_ORDER[index % 24]),
    )
    .expect("complete non-identity cycle left operand");
    let cycle_right = CliffordString::from_gates(
        (0..identity_width).map(|index| STIM_ALL_CLIFFORDS_ORDER[1 + (index / 24) % 23]),
    )
    .expect("complete non-identity cycle right operand");
    let cycle_right_before = cycle_right.clone();
    let expected_cycle = (0..identity_width)
        .map(|index| {
            STIM_ALL_CLIFFORDS_ORDER[index % 24]
                .multiply(STIM_ALL_CLIFFORDS_ORDER[1 + (index / 24) % 23])
                .expect("single-qubit Clifford product")
        })
        .collect::<Vec<_>>();
    cycle_left
        .right_multiply_in_place(&cycle_right)
        .expect("complete non-identity cycle multiplication");
    for (index, expected) in expected_cycle.into_iter().enumerate() {
        assert_eq!(
            cycle_left.gate_at(index),
            Some(expected),
            "cycle position {index}"
        );
    }
    assert_eq!(cycle_right, cycle_right_before);

    let mut first_rng = SmallRng::seed_from_u64(0x0051_ab1e);
    let mut second_rng = SmallRng::seed_from_u64(0x0051_ab1e);
    let mut first = CliffordString::random(32, &mut first_rng).expect("random Clifford string");
    let mut second = CliffordString::random(32, &mut second_rng).expect("random Clifford string");
    assert_eq!(first, second);
    first.randomize(&mut first_rng);
    second.randomize(&mut second_rng);
    assert_eq!(first, second);
}

#[test]
fn cq2_algebra_flow_value_contract_canonicalizes_and_reports_typed_errors() {
    let negative = FlowMeasurementIndex::new(-3);
    let absolute = FlowMeasurementIndex::new(5);
    assert_eq!(negative.get(), -3);
    assert_eq!(absolute.get(), 5);
    assert!(negative < absolute);

    let input = PauliString::from_str("X_").expect("flow input");
    let output = PauliString::from_str("_Z").expect("flow output");
    let value = Flow::new(
        input.clone(),
        output.clone(),
        [-3, 5, -3, 2, 2],
        [7, 1, 7, 3],
    )
    .expect("bounded Flow");
    assert_eq!(value.input(), &input);
    assert_eq!(value.output(), &output);
    assert_eq!(value.measurements().collect::<Vec<_>>(), vec![5]);
    assert_eq!(value.observables().collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(
        value.to_string(),
        "X_ -> _Z xor rec[5] xor obs[1] xor obs[3]"
    );
    assert_eq!(Flow::from_str(&value.to_string()), Ok(value.clone()));
    assert!(Flow::from_str("1 -> 1").expect("identity flow") < value);

    let x = Flow::from_str("X -> X").expect("X flow");
    let z = Flow::from_str("Z -> Z").expect("Z flow");
    assert_eq!(x.multiply(&z).expect("flow product").to_string(), "Y -> Y");
    let bad_left = Flow::from_str("1 -> X").expect("bad left");
    let bad_right = Flow::from_str("1 -> Y").expect("bad right");
    assert_eq!(
        bad_left.multiply(&bad_right),
        Err(StabilizerError::InvalidFlowProduct {
            left: "1 -> X".to_owned(),
            right: "1 -> Y".to_owned(),
        })
    );
    assert_eq!(
        Flow::from_str("iX -> X"),
        Err(StabilizerError::AntiHermitianFlow)
    );
    assert_eq!(
        Flow::from_str("X > X"),
        Err(StabilizerError::InvalidFlowText {
            text: "X > X".to_owned(),
        })
    );
}

#[test]
fn cq2_algebra_error_and_resource_contract_is_exhaustive() {
    let errors = vec![
        (
            StabilizerError::Bit(BitError::BitIndexOutOfRange { index: 7, len: 3 }),
            "bit index 7 is outside length 3",
        ),
        (
            StabilizerError::LengthMismatch { left: 2, right: 3 },
            "Pauli string length mismatch: left=2 right=3",
        ),
        (
            StabilizerError::ResourceLimitExceeded {
                resource: StabilizerResource::TableauQubits,
                requested: 513,
                limit: 512,
            },
            "Tableau qubits request 513 exceeds limit 512",
        ),
        (
            StabilizerError::ResourceSizeOverflow {
                resource: StabilizerResource::CliffordQubits,
                item_count: 2,
                repetitions: usize::MAX,
            },
            "Clifford qubits size overflowed while repeating 2 item(s) 18446744073709551615 time(s)",
        ),
        (
            StabilizerError::InvalidPauliCharacter {
                character: '?',
                offset: 4,
            },
            "unrecognized Pauli character '?' at offset 4",
        ),
        (
            StabilizerError::InvalidSparsePauliString {
                text: "X*".to_owned(),
            },
            "invalid sparse Pauli string shorthand \"X*\"",
        ),
        (
            StabilizerError::ImaginaryProduct {
                phase: PauliPhase::PlusI,
            },
            "Pauli product has imaginary phase +i",
        ),
        (
            StabilizerError::InvalidSingleQubitCliffordGate {
                gate: "CX".to_owned(),
            },
            "gate CX is not a single-qubit Clifford gate",
        ),
        (
            StabilizerError::CliffordIndexOutOfRange { index: 3, len: 2 },
            "Clifford index 3 is outside length 2",
        ),
        (
            StabilizerError::InvalidSingleQubitCliffordProduct,
            "invalid single-qubit Clifford product",
        ),
        (
            StabilizerError::TableauIndexOutOfRange { index: 4, len: 2 },
            "Tableau index 4 is outside length 2",
        ),
        (
            StabilizerError::DuplicateTableauTarget { target: 8 },
            "duplicate Tableau target 8",
        ),
        (
            StabilizerError::InvalidCommutingPauliIteratorQubitCount { num_qubits: 0 },
            "commuting Pauli string iteration requires 1..64 qubits but got 0",
        ),
        (
            StabilizerError::InvalidTableauIteratorQubitCount { num_qubits: 64 },
            "Tableau iteration requires fewer than 64 qubits but got 64",
        ),
        (
            StabilizerError::InvalidFlowText {
                text: "bad".to_owned(),
            },
            "invalid stabilizer flow text \"bad\"",
        ),
        (
            StabilizerError::AntiHermitianFlow,
            "anti-Hermitian stabilizer flows are not allowed",
        ),
        (
            StabilizerError::InvalidFlowProduct {
                left: "X -> X".to_owned(),
                right: "Z -> X".to_owned(),
            },
            "stabilizer flow product anticommutes: X -> X with Z -> X",
        ),
        (
            StabilizerError::NotPauliProduct,
            "Tableau is not a Pauli product",
        ),
        (
            StabilizerError::InvalidTableauInverse,
            "failed to derive inverse Tableau row",
        ),
        (
            StabilizerError::AntiCommutingStabilizer {
                stabilizer: "+X".to_owned(),
                conflict: "+Z".to_owned(),
            },
            "stabilizer +X anticommutes with earlier stabilizer +Z",
        ),
        (
            StabilizerError::RedundantStabilizer {
                stabilizer: "+XX".to_owned(),
            },
            "redundant stabilizer +XX is not allowed",
        ),
        (
            StabilizerError::InconsistentStabilizer {
                stabilizer: "-II".to_owned(),
            },
            "stabilizer -II has an inconsistent sign",
        ),
        (
            StabilizerError::OverconstrainedStabilizers {
                independent: 3,
                num_qubits: 2,
            },
            "stabilizer set has 3 independent generators but 2 qubits",
        ),
        (
            StabilizerError::UnderconstrainedStabilizers {
                independent: 1,
                num_qubits: 2,
            },
            "stabilizer set has 1 independent generators but 2 qubits and underconstrained conversion is disabled",
        ),
        (
            StabilizerError::InvalidStabilizerTableauSynthesis,
            "failed to synthesize a stabilizer Tableau",
        ),
        (
            StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height: 3 },
            "unitary matrix height must be a non-zero power of 2, got 3",
        ),
        (
            StabilizerError::UnitaryMatrixRowWidthMismatch {
                row: 1,
                width: 3,
                height: 4,
            },
            "unitary matrix row 1 had width 3, expected square width 4",
        ),
        (StabilizerError::MatrixNotUnitary, "matrix is not unitary"),
        (
            StabilizerError::UnitaryMatrixNotClifford,
            "unitary matrix is not a Clifford operation",
        ),
    ];
    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected, "{}", error_variant(&error));
    }

    let source_error = StabilizerError::from(BitError::BitIndexOutOfRange { index: 2, len: 1 });
    assert!(source_error.source().is_none());
    let result: StabilizerResult<()> = Err(source_error.clone());
    assert_eq!(result, Err(source_error));

    for (resource, limit, text) in [
        (StabilizerResource::PauliQubits, 1_048_576, "Pauli qubits"),
        (
            StabilizerResource::CliffordQubits,
            1_048_576,
            "Clifford qubits",
        ),
        (StabilizerResource::TableauQubits, 512, "Tableau qubits"),
        (
            StabilizerResource::RandomTableauQubits,
            64,
            "random Tableau qubits",
        ),
        (
            StabilizerResource::StabilizerSolveQubits,
            512,
            "stabilizer-solve qubits",
        ),
        (
            StabilizerResource::UnitaryMatrixDimension,
            64,
            "unitary matrix dimension",
        ),
        (
            StabilizerResource::FlowClassicalTerms,
            65_536,
            "flow classical terms",
        ),
        (
            StabilizerResource::CircuitTableauRepeatWork,
            16_777_216,
            "circuit Tableau repeat work units",
        ),
    ] {
        assert_eq!(resource.limit(), limit);
        assert_eq!(resource.to_string(), text);
    }
}

fn error_variant(error: &StabilizerError) -> &'static str {
    match error {
        StabilizerError::Bit(_) => "Bit",
        StabilizerError::LengthMismatch { .. } => "LengthMismatch",
        StabilizerError::ResourceLimitExceeded { .. } => "ResourceLimitExceeded",
        StabilizerError::ResourceSizeOverflow { .. } => "ResourceSizeOverflow",
        StabilizerError::InvalidPauliCharacter { .. } => "InvalidPauliCharacter",
        StabilizerError::InvalidSparsePauliString { .. } => "InvalidSparsePauliString",
        StabilizerError::ImaginaryProduct { .. } => "ImaginaryProduct",
        StabilizerError::InvalidSingleQubitCliffordGate { .. } => "InvalidSingleQubitCliffordGate",
        StabilizerError::CliffordIndexOutOfRange { .. } => "CliffordIndexOutOfRange",
        StabilizerError::InvalidSingleQubitCliffordProduct => "InvalidSingleQubitCliffordProduct",
        StabilizerError::InconsistentCliffordStringMetadata => "InconsistentCliffordStringMetadata",
        StabilizerError::TableauIndexOutOfRange { .. } => "TableauIndexOutOfRange",
        StabilizerError::DuplicateTableauTarget { .. } => "DuplicateTableauTarget",
        StabilizerError::InvalidCommutingPauliIteratorQubitCount { .. } => {
            "InvalidCommutingPauliIteratorQubitCount"
        }
        StabilizerError::InvalidTableauIteratorQubitCount { .. } => {
            "InvalidTableauIteratorQubitCount"
        }
        StabilizerError::InvalidFlowText { .. } => "InvalidFlowText",
        StabilizerError::AntiHermitianFlow => "AntiHermitianFlow",
        StabilizerError::InvalidFlowProduct { .. } => "InvalidFlowProduct",
        StabilizerError::NotPauliProduct => "NotPauliProduct",
        StabilizerError::InvalidTableauInverse => "InvalidTableauInverse",
        StabilizerError::AntiCommutingStabilizer { .. } => "AntiCommutingStabilizer",
        StabilizerError::RedundantStabilizer { .. } => "RedundantStabilizer",
        StabilizerError::InconsistentStabilizer { .. } => "InconsistentStabilizer",
        StabilizerError::OverconstrainedStabilizers { .. } => "OverconstrainedStabilizers",
        StabilizerError::UnderconstrainedStabilizers { .. } => "UnderconstrainedStabilizers",
        StabilizerError::InvalidStabilizerTableauSynthesis => "InvalidStabilizerTableauSynthesis",
        StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { .. } => {
            "UnitaryMatrixHeightNotPowerOfTwo"
        }
        StabilizerError::UnitaryMatrixRowWidthMismatch { .. } => "UnitaryMatrixRowWidthMismatch",
        StabilizerError::MatrixNotUnitary => "MatrixNotUnitary",
        StabilizerError::UnitaryMatrixNotClifford => "UnitaryMatrixNotClifford",
    }
}
