#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "generated round-trip properties report the exact rendered circuit on failure"
)]

//! Generated `.stim` parse/print properties.
//!
//! Each case renders an independently generated circuit tree with randomized surface syntax
//! (gate aliases, keyword case, spacing, float spelling, comments, CRLF, escaped and non-UTF-8
//! tags, inverted targets, Pauli products, `rec[-k]` and `sweep[k]` controls, nested `REPEAT`
//! blocks). The parsed model must match the tree through an independent schema-v1 fingerprint
//! encoding, and the canonical printer must be a parse/print fixed point. Stim prints circuit
//! arguments with six significant digits, so exact model equality across printing is required
//! only when every generated value is six-digit representable.

mod fingerprint_schema;

use fingerprint_schema::{CIRCUIT_DIALECT, SchemaHead, SchemaItem, SchemaTarget, schema_digest};
use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_model::Circuit;

const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;
const MAX_TARGET_VALUE: u32 = (1 << 24) - 1;

#[derive(Clone, Copy, Debug)]
enum GenTarget {
    Qubit {
        id: u32,
        inverted: bool,
    },
    /// `rec[-k]`; `k == 0` spells Stim's retained `rec[-0]`.
    Record(u32),
    Sweep(u32),
    Pauli {
        pauli: u8,
        id: u32,
        inverted: bool,
    },
    Combiner,
}

#[derive(Clone, Debug)]
struct GenOp {
    canonical: &'static str,
    spelling: &'static str,
    style: u8,
    tag: Option<Vec<u8>>,
    args: Vec<f64>,
    targets: Vec<GenTarget>,
}

#[derive(Clone, Debug)]
enum GenItem {
    Op(GenOp),
    Repeat {
        style: u8,
        tag: Option<Vec<u8>>,
        count: u64,
        body: Vec<GenItem>,
    },
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    crlf: bool,
    indent: bool,
    comments: bool,
}

/// Value domain for generated floating-point arguments.
#[derive(Clone, Copy, Debug)]
enum Values {
    /// Every value survives Stim's six-significant-digit circuit printer exactly.
    SixDigitExact,
    /// Arbitrary finite values, including subnormals and extremes.
    Arbitrary,
}

fn probability(max_thousandths: u32, values: Values) -> BoxedStrategy<f64> {
    let max = f64::from(max_thousandths) / 1000.0;
    match values {
        Values::SixDigitExact => (0..=max_thousandths * 1000)
            .prop_map(|value| f64::from(value) / 1_000_000.0)
            .boxed(),
        Values::Arbitrary => prop_oneof![
            1 => Just(0.0),
            1 => Just(max),
            1 => Just(f64::from_bits(1)),
            1 => prop::num::f64::POSITIVE | prop::num::f64::SUBNORMAL,
            4 => 0.0..=max,
        ]
        .prop_filter("probability inside the gate's domain", move |value| {
            (0.0..=max).contains(value)
        })
        .boxed(),
    }
}

fn coordinate(values: Values) -> BoxedStrategy<f64> {
    match values {
        Values::SixDigitExact => prop_oneof![
            4 => (-64_i32..=64).prop_map(|value| f64::from(value) / 4.0),
            1 => Just(-0.0),
            1 => (-999_999_i32..=999_999).prop_map(f64::from),
            2 => (-999_999_i32..=999_999).prop_map(|value| f64::from(value) / 1000.0),
        ]
        .boxed(),
        Values::Arbitrary => prop_oneof![
            2 => (-16_i32..=16).prop_map(f64::from),
            1 => Just(-0.0),
            1 => Just(f64::MAX),
            1 => Just(f64::MIN),
            1 => Just(f64::from_bits(1)),
            3 => prop::num::f64::NORMAL | prop::num::f64::SUBNORMAL | prop::num::f64::ZERO,
            2 => -1.0e9..1.0e9_f64,
        ]
        .boxed(),
    }
}

fn tag() -> BoxedStrategy<Option<Vec<u8>>> {
    let tag_byte = prop_oneof![
        3 => prop::sample::select(b"]\\\r\n#[ \t(),*!{}".to_vec()),
        3 => prop::sample::select(b"abcXYZ019_".to_vec()),
        1 => any::<u8>(),
    ];
    let utf8_text = prop::collection::vec(any::<char>(), 1..4)
        .prop_map(|chars| chars.into_iter().collect::<String>().into_bytes());
    prop_oneof![
        4 => Just(None),
        1 => Just(Some(Vec::new())),
        3 => prop::collection::vec(tag_byte, 1..10).prop_map(Some),
        1 => utf8_text.prop_map(Some),
    ]
    .boxed()
}

fn qubit() -> BoxedStrategy<u32> {
    prop_oneof![
        8 => 0_u32..32,
        1 => Just(MAX_TARGET_VALUE),
        1 => 0..=MAX_TARGET_VALUE,
    ]
    .boxed()
}

fn qubit_pair() -> BoxedStrategy<(u32, u32)> {
    (qubit(), qubit())
        .prop_filter("pair targets must differ", |(left, right)| left != right)
        .boxed()
}

fn lookback() -> BoxedStrategy<u32> {
    prop_oneof![
        8 => 1_u32..=16,
        1 => Just(0),
        1 => Just(MAX_TARGET_VALUE),
        1 => 1..=MAX_TARGET_VALUE,
    ]
    .boxed()
}

fn sweep() -> BoxedStrategy<u32> {
    prop_oneof![4 => 0_u32..16, 1 => 0..=MAX_TARGET_VALUE].boxed()
}

fn pauli_kind() -> BoxedStrategy<u8> {
    (1_u8..=3).boxed()
}

/// One Pauli product over distinct qubits, without combiners.
fn pauli_product(allow_inversion: bool) -> BoxedStrategy<Vec<GenTarget>> {
    prop::collection::vec((pauli_kind(), qubit(), any::<bool>()), 1..4)
        .prop_map(move |factors| {
            let mut seen = Vec::new();
            factors
                .into_iter()
                .filter(|(_, id, _)| {
                    let fresh = !seen.contains(id);
                    seen.push(*id);
                    fresh
                })
                .map(|(pauli, id, inverted)| GenTarget::Pauli {
                    pauli,
                    id,
                    inverted: allow_inversion && inverted,
                })
                .collect()
        })
        .boxed()
}

fn op(
    names: &'static [&'static str],
    args: BoxedStrategy<Vec<f64>>,
    targets: BoxedStrategy<Vec<GenTarget>>,
) -> BoxedStrategy<GenOp> {
    let canonical = names
        .first()
        .copied()
        .expect("gate names include a canonical name");
    (
        prop::sample::select(names),
        any::<u8>(),
        tag(),
        args,
        targets,
    )
        .prop_map(move |(spelling, style, tag, args, targets)| GenOp {
            canonical,
            spelling,
            style,
            tag,
            args,
            targets,
        })
        .boxed()
}

fn no_args() -> BoxedStrategy<Vec<f64>> {
    Just(Vec::new()).boxed()
}

fn qubits(inversion: bool) -> BoxedStrategy<Vec<GenTarget>> {
    prop::collection::vec((qubit(), any::<bool>()), 1..5)
        .prop_map(move |targets| {
            targets
                .into_iter()
                .map(|(id, inverted)| GenTarget::Qubit {
                    id,
                    inverted: inversion && inverted,
                })
                .collect()
        })
        .boxed()
}

fn pairs(inversion: bool) -> BoxedStrategy<Vec<GenTarget>> {
    prop::collection::vec((qubit_pair(), any::<bool>(), any::<bool>()), 1..3)
        .prop_map(move |pairs| {
            pairs
                .into_iter()
                .flat_map(|((left, right), left_inverted, right_inverted)| {
                    [
                        GenTarget::Qubit {
                            id: left,
                            inverted: inversion && left_inverted,
                        },
                        GenTarget::Qubit {
                            id: right,
                            inverted: inversion && right_inverted,
                        },
                    ]
                })
                .collect()
        })
        .boxed()
}

fn classical_control_pairs(symmetric: bool) -> BoxedStrategy<Vec<GenTarget>> {
    let control = prop_oneof![
        lookback()
            .prop_filter("rec[-0] is not a control bit", |value| *value != 0)
            .prop_map(GenTarget::Record),
        sweep().prop_map(GenTarget::Sweep),
    ];
    prop::collection::vec((control, qubit(), any::<bool>()), 1..3)
        .prop_map(move |pairs| {
            pairs
                .into_iter()
                .flat_map(|(control, target, swap)| {
                    let target = GenTarget::Qubit {
                        id: target,
                        inverted: false,
                    };
                    if symmetric && swap {
                        [target, control]
                    } else {
                        [control, target]
                    }
                })
                .collect()
        })
        .boxed()
}

fn records(min: usize) -> BoxedStrategy<Vec<GenTarget>> {
    prop::collection::vec(lookback().prop_map(GenTarget::Record), min..4).boxed()
}

fn coordinates(values: Values, min: usize) -> BoxedStrategy<Vec<f64>> {
    prop::collection::vec(coordinate(values), min..4).boxed()
}

fn optional_probability(max_thousandths: u32, values: Values) -> BoxedStrategy<Vec<f64>> {
    prop::option::of(probability(max_thousandths, values))
        .prop_map(|value| value.into_iter().collect())
        .boxed()
}

fn generated_op(values: Values) -> BoxedStrategy<GenOp> {
    let p = |max| {
        probability(max, values)
            .prop_map(|value| vec![value])
            .boxed()
    };
    let products = prop::collection::vec(pauli_product(true), 1..4)
        .prop_map(|products| {
            let mut targets = Vec::new();
            for product in products {
                for (index, factor) in product.into_iter().enumerate() {
                    if index > 0 {
                        targets.push(GenTarget::Combiner);
                    }
                    targets.push(factor);
                }
            }
            targets
        })
        .boxed();
    let observable_targets = prop_oneof![records(1), pauli_product(false),].boxed();
    prop_oneof![
        op(&["H", "H_XZ"], no_args(), qubits(false)),
        op(&["S", "SQRT_Z"], no_args(), qubits(false)),
        op(&["S_DAG", "SQRT_Z_DAG"], no_args(), qubits(false)),
        op(&["SQRT_X"], no_args(), qubits(false)),
        op(&["C_XYZ"], no_args(), qubits(false)),
        op(&["I"], no_args(), qubits(false)),
        op(&["CX", "CNOT", "ZCX"], no_args(), pairs(false)),
        op(&["SWAP"], no_args(), pairs(false)),
        op(&["ISWAP"], no_args(), pairs(false)),
        op(
            &["CX", "CNOT", "ZCX"],
            no_args(),
            classical_control_pairs(false)
        ),
        op(&["CY", "ZCY"], no_args(), classical_control_pairs(false)),
        op(&["CZ", "ZCZ"], no_args(), classical_control_pairs(true)),
        op(&["X_ERROR"], p(1000), qubits(false)),
        op(&["Z_ERROR"], p(1000), qubits(false)),
        op(&["DEPOLARIZE1"], p(750), qubits(false)),
        op(&["DEPOLARIZE2"], p(937), pairs(false)),
        op(&["HERALDED_ERASE"], p(1000), qubits(false)),
        op(
            &["PAULI_CHANNEL_1"],
            prop::collection::vec(probability(300, values), 3).boxed(),
            qubits(false),
        ),
        op(&["E", "CORRELATED_ERROR"], p(1000), pauli_product(false)),
        op(&["ELSE_CORRELATED_ERROR"], p(1000), pauli_product(false)),
        op(
            &["M", "MZ"],
            optional_probability(1000, values),
            qubits(true)
        ),
        op(&["MX"], optional_probability(1000, values), qubits(true)),
        op(
            &["MR", "MRZ"],
            optional_probability(1000, values),
            qubits(true)
        ),
        op(&["MXX"], optional_probability(1000, values), pairs(true)),
        op(&["MPP"], optional_probability(1000, values), products),
        op(&["R", "RZ"], no_args(), qubits(false)),
        op(&["RX"], no_args(), qubits(false)),
        op(
            &["MPAD"],
            no_args(),
            prop::collection::vec(
                (0_u32..=1).prop_map(|id| GenTarget::Qubit {
                    id,
                    inverted: false
                }),
                1..4
            )
            .boxed(),
        ),
        op(&["DETECTOR"], coordinates(values, 0), records(0)),
        op(
            &["OBSERVABLE_INCLUDE"],
            (0_u32..8).prop_map(|id| vec![f64::from(id)]).boxed(),
            observable_targets,
        ),
        op(&["QUBIT_COORDS"], coordinates(values, 1), qubits(false)),
        op(
            &["SHIFT_COORDS"],
            coordinates(values, 0),
            Just(Vec::new()).boxed()
        ),
        op(&["TICK"], no_args(), Just(Vec::new()).boxed()),
    ]
    .boxed()
}

fn circuit_tree(values: Values) -> impl Strategy<Value = Vec<GenItem>> {
    let item = generated_op(values)
        .prop_map(GenItem::Op)
        .prop_recursive(3, 64, 6, |inner| {
            (
                any::<u8>(),
                tag(),
                prop_oneof![4 => 1_u64..5, 1 => 1_u64..=1_000_000_000_000],
                prop::collection::vec(inner, 0..6),
            )
                .prop_map(|(style, tag, count, body)| GenItem::Repeat {
                    style,
                    tag,
                    count,
                    body,
                })
        });
    prop::collection::vec(item, 0..12)
}

fn layout() -> impl Strategy<Value = Layout> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(crlf, indent, comments)| Layout {
        crlf,
        indent,
        comments,
    })
}

fn render(items: &[GenItem], layout: Layout) -> Vec<u8> {
    let mut out = Vec::new();
    if layout.comments {
        out.extend_from_slice(b"# generated ] \\ [ comment");
        end_line(&mut out, layout);
    }
    render_items(&mut out, items, layout, 0);
    out
}

fn end_line(out: &mut Vec<u8>, layout: Layout) {
    out.extend_from_slice(if layout.crlf { b"\r\n" } else { b"\n" });
}

fn indent(out: &mut Vec<u8>, layout: Layout, depth: usize) {
    if layout.indent {
        out.extend(std::iter::repeat_n(b' ', depth * 4));
    }
}

fn render_items(out: &mut Vec<u8>, items: &[GenItem], layout: Layout, depth: usize) {
    for item in items {
        indent(out, layout, depth);
        let style = match item {
            GenItem::Op(op) => {
                render_name(out, op.spelling, op.style, op.tag.as_deref());
                render_args(out, &op.args, op.style);
                for target in &op.targets {
                    if !matches!(target, GenTarget::Combiner) {
                        out.extend_from_slice(if op.style & 0x40 != 0 { b"\t" } else { b" " });
                    }
                    render_target(out, *target, op.style);
                }
                op.style
            }
            GenItem::Repeat {
                style,
                tag,
                count,
                body,
            } => {
                render_name(out, "REPEAT", *style, tag.as_deref());
                out.extend_from_slice(format!(" {count} {{").as_bytes());
                end_line(out, layout);
                render_items(out, body, layout, depth + 1);
                indent(out, layout, depth);
                out.push(b'}');
                *style
            }
        };
        if layout.comments && style & 0x80 != 0 {
            out.extend_from_slice(b" # trailing ] \\x comment");
        }
        end_line(out, layout);
    }
}

fn render_name(out: &mut Vec<u8>, name: &str, style: u8, tag: Option<&[u8]>) {
    if style & 1 == 0 {
        out.extend_from_slice(name.as_bytes());
    } else {
        out.extend_from_slice(name.to_ascii_lowercase().as_bytes());
    }
    if let Some(tag) = tag {
        out.push(b'[');
        for byte in tag {
            match byte {
                b']' => out.extend_from_slice(b"\\C"),
                b'\\' => out.extend_from_slice(b"\\B"),
                b'\r' => out.extend_from_slice(b"\\r"),
                b'\n' => out.extend_from_slice(b"\\n"),
                byte => out.push(*byte),
            }
        }
        out.push(b']');
    }
}

fn render_args(out: &mut Vec<u8>, args: &[f64], style: u8) {
    if args.is_empty() {
        return;
    }
    out.push(b'(');
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(if style & 0b10 != 0 { b", " } else { b"," });
        }
        let mut text = match (style >> 2) & 0b11 {
            0 => format!("{arg}"),
            1 => format!("{arg:?}"),
            2 => format!("{arg:e}"),
            _ => format!("{arg:E}"),
        };
        if text.len() > MAX_STIM_NUMBER_TOKEN_BYTES {
            text = format!("{arg:e}");
        }
        out.extend_from_slice(text.as_bytes());
    }
    out.push(b')');
}

fn render_target(out: &mut Vec<u8>, target: GenTarget, style: u8) {
    let lower = style & 0x20 != 0;
    let text = match target {
        GenTarget::Qubit { id, inverted } => format!("{}{id}", if inverted { "!" } else { "" }),
        // Stim's `rec[` and `sweep[` prefixes are case-sensitive.
        GenTarget::Record(lookback) => format!("rec[-{lookback}]"),
        GenTarget::Sweep(id) => format!("sweep[{id}]"),
        GenTarget::Pauli {
            pauli,
            id,
            inverted,
        } => {
            let letter = match (pauli, lower) {
                (1, false) => 'X',
                (2, false) => 'Y',
                (3, false) => 'Z',
                (1, true) => 'x',
                (2, true) => 'y',
                (_, true) => 'z',
                _ => 'Z',
            };
            format!("{}{letter}{id}", if inverted { "!" } else { "" })
        }
        GenTarget::Combiner => "*".to_string(),
    };
    out.extend_from_slice(text.as_bytes());
}

fn expected_tag(tag: Option<&Vec<u8>>) -> Option<Vec<u8>> {
    tag.filter(|tag| !tag.is_empty()).cloned()
}

fn schema_items(items: &[GenItem]) -> Vec<SchemaItem> {
    items
        .iter()
        .map(|item| match item {
            GenItem::Op(op) => SchemaItem::Instruction {
                head: SchemaHead::Gate(op.canonical),
                args: op.args.clone(),
                targets: op
                    .targets
                    .iter()
                    .map(|target| match *target {
                        GenTarget::Qubit { id, inverted } => SchemaTarget::Qubit { id, inverted },
                        GenTarget::Record(lookback) => SchemaTarget::Record {
                            offset: -i32::try_from(lookback).expect("lookback fits i32"),
                            negative_zero: lookback == 0,
                        },
                        GenTarget::Sweep(id) => SchemaTarget::Sweep(id),
                        GenTarget::Pauli {
                            pauli,
                            id,
                            inverted,
                        } => SchemaTarget::Pauli {
                            pauli,
                            id,
                            inverted,
                        },
                        GenTarget::Combiner => SchemaTarget::Combiner,
                    })
                    .collect(),
                tag: expected_tag(op.tag.as_ref()),
            },
            GenItem::Repeat {
                tag, count, body, ..
            } => SchemaItem::Repeat {
                count: *count,
                tag: expected_tag(tag.as_ref()),
                body: schema_items(body),
            },
        })
        .collect()
}

fn config(seed: u64) -> Config {
    Config {
        cases: 512,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    }
}

/// Gates Stim never fuses with an adjacent identical instruction.
const NOT_FUSABLE: &[&str] = &[
    "DETECTOR",
    "OBSERVABLE_INCLUDE",
    "TICK",
    "QUBIT_COORDS",
    "SHIFT_COORDS",
    "E",
    "ELSE_CORRELATED_ERROR",
];

/// Applies Stim's parse-time fusion: adjacent instructions with the same gate, numerically equal
/// arguments, and the same tag merge their target lists unless the gate is not fusable.
fn fused(items: &[GenItem]) -> Vec<GenItem> {
    let mut out: Vec<GenItem> = Vec::new();
    for item in items {
        match item {
            GenItem::Op(op) => {
                if let Some(GenItem::Op(previous)) = out.last_mut()
                    && previous.canonical == op.canonical
                    && previous.args == op.args
                    && expected_tag(previous.tag.as_ref()) == expected_tag(op.tag.as_ref())
                    && !NOT_FUSABLE.contains(&op.canonical)
                {
                    previous.targets.extend_from_slice(&op.targets);
                    continue;
                }
                out.push(item.clone());
            }
            GenItem::Repeat {
                style,
                tag,
                count,
                body,
            } => out.push(GenItem::Repeat {
                style: *style,
                tag: tag.clone(),
                count: *count,
                body: fused(body),
            }),
        }
    }
    out
}

fn parse(input: &[u8], context: &str) -> Circuit {
    Circuit::from_stim_bytes(input).unwrap_or_else(|error| {
        panic!(
            "{context} failed to parse: {error}\n{}",
            String::from_utf8_lossy(input)
        )
    })
}

/// Parses rendered input, checks it against the generated tree, and returns the parsed circuit
/// with its first two canonical prints.
///
/// Stim prints non-integral circuit arguments with six significant digits but integral ones in
/// full, so `-782016374.46` prints as `-7.82016e+08`, which reparses as an integer and then
/// prints as `-782016000`. The canonical text is therefore a fixed point from the second print
/// onward, and from the first print when every value is six-digit representable.
fn round_trip(items: &[GenItem], input: &[u8]) -> (Circuit, Vec<u8>, Vec<u8>) {
    let shown = String::from_utf8_lossy(input).into_owned();
    let parsed = parse(input, "generated circuit");
    assert_eq!(
        parsed.fingerprint().digest(),
        schema_digest(CIRCUIT_DIALECT, &schema_items(&fused(items))),
        "parsed circuit differs from the generated tree\n{shown}"
    );

    let first = parsed.to_stim_bytes();
    let second = parse(&first, "first canonical print").to_stim_bytes();
    let third = parse(&second, "second canonical print").to_stim_bytes();
    assert_eq!(
        String::from_utf8_lossy(&third),
        String::from_utf8_lossy(&second),
        "canonical printer is not a fixed point\ninput:\n{shown}"
    );

    if let Ok(text) = std::str::from_utf8(input) {
        let from_str = Circuit::from_stim_str(text)
            .unwrap_or_else(|error| panic!("UTF-8 circuit failed through from_stim_str: {error}"));
        assert!(from_str == parsed, "str and byte parsers disagree\n{shown}");
        assert_eq!(from_str.to_stim_string().into_bytes(), first);
    }
    (parsed, first, second)
}

proptest! {
    #![proptest_config(config(0x5a17_5eed_cafe_f00d))]

    #[test]
    fn six_digit_circuits_survive_print_and_reparse_exactly(
        items in circuit_tree(Values::SixDigitExact),
        layout in layout(),
    ) {
        let input = render(&items, layout);
        let (parsed, first, second) = round_trip(&items, &input);
        prop_assert_eq!(
            String::from_utf8_lossy(&second),
            String::from_utf8_lossy(&first),
            "six-digit circuit text is not a first-print fixed point"
        );
        let reparsed = parse(&first, "canonical circuit");
        prop_assert!(
            reparsed == parsed,
            "six-digit circuit changed through canonical text\n{}",
            String::from_utf8_lossy(&input)
        );
        prop_assert_eq!(reparsed.fingerprint(), parsed.fingerprint());
    }

    #[test]
    fn arbitrary_value_circuits_print_to_a_fixed_point(
        items in circuit_tree(Values::Arbitrary),
        layout in layout(),
    ) {
        let input = render(&items, layout);
        round_trip(&items, &input);
    }
}
