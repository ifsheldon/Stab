#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "generated round-trip properties report the exact rendered model on failure"
)]

//! Generated `.dem` print/parse properties.
//!
//! Each case renders an independently generated DEM tree with randomized surface syntax
//! (keyword case, spacing, float spelling, comments, CRLF, tag escapes, and non-UTF-8 tag bytes),
//! then requires the parsed model to match the tree bit-for-bit, the canonical printer to be a
//! parse/print fixed point, and the model fingerprint to match an independent schema encoding.

mod fingerprint_schema;

use fingerprint_schema::{DEM_DIALECT, SchemaHead, SchemaItem, SchemaTarget, schema_digest};
use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_model::{
    DemInstructionKind, DemItem, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel,
};

/// Pinned Stim rejects number tokens longer than 63 bytes.
const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;

/// Stim's DEM text parser reads detector ids, shifts, and repeat counts as 60-bit integers even though
/// programmatic construction admits wider values; generated text stays inside the parser's domain.
const MAX_PARSED_DEM_INTEGER: u64 = (1 << 60) - 1;

#[derive(Clone, Debug)]
enum GenTarget {
    Detector(u64),
    Observable(u32),
}

#[derive(Clone, Debug)]
enum GenDem {
    Error {
        style: u8,
        tag: Option<Vec<u8>>,
        probability: f64,
        groups: Vec<Vec<GenTarget>>,
    },
    Detector {
        style: u8,
        tag: Option<Vec<u8>>,
        coordinates: Vec<f64>,
        detector: u64,
    },
    Observable {
        style: u8,
        tag: Option<Vec<u8>>,
        observable: u32,
    },
    Shift {
        style: u8,
        tag: Option<Vec<u8>>,
        coordinates: Vec<f64>,
        shift: u64,
    },
    Repeat {
        style: u8,
        tag: Option<Vec<u8>>,
        count: u64,
        body: Vec<GenDem>,
    },
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    crlf: bool,
    indent_repeat_bodies: bool,
    blank_lines: bool,
}

fn finite_coordinate() -> BoxedStrategy<f64> {
    prop_oneof![
        3 => (-16_i32..=16).prop_map(f64::from),
        1 => Just(-0.0),
        1 => Just(f64::MAX),
        1 => Just(f64::MIN),
        1 => Just(f64::MIN_POSITIVE),
        1 => Just(f64::from_bits(1)),
        1 => Just(-f64::from_bits(1)),
        3 => prop::num::f64::NORMAL | prop::num::f64::SUBNORMAL | prop::num::f64::ZERO,
        2 => -1.0e6..1.0e6_f64,
    ]
    .boxed()
}

fn probability() -> BoxedStrategy<f64> {
    prop_oneof![
        1 => Just(0.0),
        1 => Just(1.0),
        1 => Just(f64::from_bits(1.0_f64.to_bits() - 1)),
        1 => Just(f64::from_bits(1)),
        1 => prop::num::f64::POSITIVE | prop::num::f64::SUBNORMAL,
        3 => 0.0..=1.0_f64,
        2 => (0_u32..=1000).prop_map(|value| f64::from(value) / 1000.0),
    ]
    .prop_filter("probabilities stay inside [0, 1]", |value| {
        (0.0..=1.0).contains(value)
    })
    .boxed()
}

fn tag() -> BoxedStrategy<Option<Vec<u8>>> {
    let tag_byte = prop_oneof![
        3 => prop::sample::select(b"]\\\r\n#[ \tC(),^{}".to_vec()),
        3 => prop::sample::select(b"abcXYZ019_".to_vec()),
        1 => any::<u8>(),
    ];
    let utf8_text = prop::collection::vec(any::<char>(), 1..4)
        .prop_map(|chars| chars.into_iter().collect::<String>().into_bytes());
    prop_oneof![
        3 => Just(None),
        1 => Just(Some(Vec::new())),
        3 => prop::collection::vec(tag_byte, 1..10).prop_map(Some),
        1 => utf8_text.prop_map(Some),
    ]
    .boxed()
}

fn detector_id() -> BoxedStrategy<u64> {
    prop_oneof![
        6 => 0_u64..64,
        1 => Just(MAX_PARSED_DEM_INTEGER),
        1 => 0..=MAX_PARSED_DEM_INTEGER,
    ]
    .boxed()
}

fn observable_id() -> BoxedStrategy<u32> {
    prop_oneof![6 => 0_u32..16, 1 => Just(u32::MAX), 1 => any::<u32>()].boxed()
}

fn error_groups() -> BoxedStrategy<Vec<Vec<GenTarget>>> {
    let target = prop_oneof![
        3 => detector_id().prop_map(GenTarget::Detector),
        1 => observable_id().prop_map(GenTarget::Observable),
    ];
    prop_oneof![
        1 => Just(Vec::new()),
        6 => prop::collection::vec(prop::collection::vec(target, 1..4), 1..4),
    ]
    .boxed()
}

fn dem_leaf() -> BoxedStrategy<GenDem> {
    let coordinates = || prop::collection::vec(finite_coordinate(), 0..4);
    prop_oneof![
        4 => (any::<u8>(), tag(), probability(), error_groups()).prop_map(
            |(style, tag, probability, groups)| GenDem::Error { style, tag, probability, groups }
        ),
        2 => (any::<u8>(), tag(), coordinates(), detector_id()).prop_map(
            |(style, tag, coordinates, detector)| GenDem::Detector {
                style,
                tag,
                coordinates,
                detector,
            }
        ),
        1 => (any::<u8>(), tag(), observable_id()).prop_map(|(style, tag, observable)| {
            GenDem::Observable { style, tag, observable }
        }),
        2 => (
            any::<u8>(),
            tag(),
            coordinates(),
            prop_oneof![4 => 0_u64..8, 1 => 0..=MAX_PARSED_DEM_INTEGER],
        )
            .prop_map(|(style, tag, coordinates, shift)| GenDem::Shift {
                style,
                tag,
                coordinates,
                shift,
            }),
    ]
    .boxed()
}

fn dem_tree() -> impl Strategy<Value = Vec<GenDem>> {
    let item = dem_leaf().prop_recursive(3, 48, 6, |inner| {
        (
            any::<u8>(),
            tag(),
            prop_oneof![4 => 0_u64..5, 1 => Just(MAX_PARSED_DEM_INTEGER), 1 => 0..=MAX_PARSED_DEM_INTEGER],
            prop::collection::vec(inner, 0..6),
        )
            .prop_map(|(style, tag, count, body)| GenDem::Repeat {
                style,
                tag,
                count,
                body,
            })
    });
    prop::collection::vec(item, 0..10)
}

fn layout() -> impl Strategy<Value = Layout> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(
        |(crlf, indent_repeat_bodies, blank_lines)| Layout {
            crlf,
            indent_repeat_bodies,
            blank_lines,
        },
    )
}

fn render(items: &[GenDem], layout: Layout) -> Vec<u8> {
    let mut out = Vec::new();
    if layout.blank_lines {
        out.extend_from_slice(b"# generated ] \\ comment");
        end_line(&mut out, layout);
    }
    render_items(&mut out, items, layout, 0);
    out
}

fn end_line(out: &mut Vec<u8>, layout: Layout) {
    out.extend_from_slice(if layout.crlf { b"\r\n" } else { b"\n" });
}

fn render_items(out: &mut Vec<u8>, items: &[GenDem], layout: Layout, depth: usize) {
    for item in items {
        if layout.indent_repeat_bodies {
            out.extend(std::iter::repeat_n(b' ', depth * 4));
        }
        match item {
            GenDem::Error {
                style,
                tag,
                probability,
                groups,
            } => {
                render_head(out, "error", *style, tag.as_deref());
                render_args(out, std::slice::from_ref(probability), *style, true);
                for (group_index, group) in groups.iter().enumerate() {
                    if group_index > 0 {
                        out.extend_from_slice(b" ^");
                    }
                    for target in group {
                        out.extend_from_slice(target_separator(*style));
                        match target {
                            GenTarget::Detector(id) => {
                                out.extend_from_slice(format!("D{id}").as_bytes());
                            }
                            GenTarget::Observable(id) => {
                                out.extend_from_slice(format!("L{id}").as_bytes());
                            }
                        }
                    }
                }
            }
            GenDem::Detector {
                style,
                tag,
                coordinates,
                detector,
            } => {
                render_head(out, "detector", *style, tag.as_deref());
                render_args(out, coordinates, *style, false);
                out.extend_from_slice(target_separator(*style));
                out.extend_from_slice(format!("D{detector}").as_bytes());
            }
            GenDem::Observable {
                style,
                tag,
                observable,
            } => {
                render_head(out, "logical_observable", *style, tag.as_deref());
                out.extend_from_slice(target_separator(*style));
                out.extend_from_slice(format!("L{observable}").as_bytes());
            }
            GenDem::Shift {
                style,
                tag,
                coordinates,
                shift,
            } => {
                render_head(out, "shift_detectors", *style, tag.as_deref());
                render_args(out, coordinates, *style, false);
                out.extend_from_slice(target_separator(*style));
                out.extend_from_slice(shift.to_string().as_bytes());
            }
            GenDem::Repeat {
                style,
                tag,
                count,
                body,
            } => {
                render_head(out, "repeat", *style, tag.as_deref());
                out.extend_from_slice(format!(" {count} {{").as_bytes());
                end_line(out, layout);
                render_items(out, body, layout, depth + 1);
                if layout.indent_repeat_bodies {
                    out.extend(std::iter::repeat_n(b' ', depth * 4));
                }
                out.push(b'}');
            }
        }
        if style_of(item) & 0x80 != 0 {
            out.extend_from_slice(b" # trailing ] \\B comment");
        }
        end_line(out, layout);
        if layout.blank_lines && style_of(item) & 0x40 != 0 {
            end_line(out, layout);
        }
    }
}

const fn style_of(item: &GenDem) -> u8 {
    match item {
        GenDem::Error { style, .. }
        | GenDem::Detector { style, .. }
        | GenDem::Observable { style, .. }
        | GenDem::Shift { style, .. }
        | GenDem::Repeat { style, .. } => *style,
    }
}

fn render_head(out: &mut Vec<u8>, name: &str, style: u8, tag: Option<&[u8]>) {
    let name = match style & 0b11 {
        0 | 1 => name.to_string(),
        2 => name.to_ascii_uppercase(),
        _ => name
            .chars()
            .enumerate()
            .map(|(index, ch)| {
                if index % 2 == 0 {
                    ch.to_ascii_uppercase()
                } else {
                    ch
                }
            })
            .collect(),
    };
    out.extend_from_slice(name.as_bytes());
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

fn render_args(out: &mut Vec<u8>, args: &[f64], style: u8, always: bool) {
    if args.is_empty() && !always {
        return;
    }
    let padded = style & 0b100 != 0;
    out.push(b'(');
    if padded {
        out.push(b' ');
    }
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(if style & 0b1000 != 0 { b", " } else { b"," });
        }
        let mut text = match (style >> 4) & 0b11 {
            0 => format!("{arg}"),
            1 => format!("{arg:?}"),
            2 => format!("{arg:e}"),
            _ => format!("{arg:E}"),
        };
        // Pinned Stim rejects number tokens longer than 63 bytes, so long positional spellings
        // of huge or tiny values fall back to exact scientific notation.
        if text.len() > MAX_STIM_NUMBER_TOKEN_BYTES {
            text = format!("{arg:e}");
        }
        out.extend_from_slice(text.as_bytes());
    }
    if padded {
        out.push(b' ');
    }
    out.push(b')');
}

fn target_separator(style: u8) -> &'static [u8] {
    match (style >> 6) & 0b11 {
        0 | 1 => b" ",
        2 => b"\t",
        _ => b"   ",
    }
}

fn expected_tag(tag: Option<&Vec<u8>>) -> Option<&[u8]> {
    tag.map(Vec::as_slice).filter(|tag| !tag.is_empty())
}

fn expected_targets(groups: &[Vec<GenTarget>]) -> Vec<DemTarget> {
    let mut targets = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        if group_index > 0 {
            targets.push(DemTarget::separator());
        }
        for target in group {
            targets.push(match target {
                GenTarget::Detector(id) => DemTarget::relative_detector(*id).expect("valid id"),
                GenTarget::Observable(id) => {
                    DemTarget::logical_observable(u64::from(*id)).expect("valid id")
                }
            });
        }
    }
    targets
}

fn assert_bitwise_args(actual: &[f64], expected: &[f64], context: &str) {
    let actual_bits = actual
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    let expected_bits = expected
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_bits, expected_bits,
        "{context}: {actual:?} vs {expected:?}"
    );
}

fn assert_model_matches(model: &DetectorErrorModel, expected: &[GenDem], context: &str) {
    assert_eq!(model.items().len(), expected.len(), "{context}: item count");
    for (index, (item, generated)) in model.items().iter().zip(expected).enumerate() {
        let context = format!("{context} item {index}");
        match (item, generated) {
            (
                DemItem::RepeatBlock(repeat),
                GenDem::Repeat {
                    tag, count, body, ..
                },
            ) => {
                assert_eq!(repeat.repeat_count().get(), *count, "{context}");
                assert_eq!(repeat.tag_bytes(), expected_tag(tag.as_ref()), "{context}");
                assert_model_matches(repeat.body(), body, &context);
            }
            (DemItem::Instruction(instruction), generated) => {
                let (kind, args, targets, tag) = match generated {
                    GenDem::Error {
                        tag,
                        probability,
                        groups,
                        ..
                    } => (
                        DemInstructionKind::Error,
                        vec![*probability],
                        expected_targets(groups),
                        tag,
                    ),
                    GenDem::Detector {
                        tag,
                        coordinates,
                        detector,
                        ..
                    } => (
                        DemInstructionKind::Detector,
                        coordinates.clone(),
                        vec![DemTarget::relative_detector(*detector).expect("valid id")],
                        tag,
                    ),
                    GenDem::Observable {
                        tag, observable, ..
                    } => (
                        DemInstructionKind::LogicalObservable,
                        Vec::new(),
                        vec![DemTarget::logical_observable(u64::from(*observable)).expect("id")],
                        tag,
                    ),
                    GenDem::Shift {
                        tag,
                        coordinates,
                        shift,
                        ..
                    } => (
                        DemInstructionKind::ShiftDetectors,
                        coordinates.clone(),
                        vec![DemTarget::numeric(*shift)],
                        tag,
                    ),
                    GenDem::Repeat { .. } => panic!("{context}: parsed instruction for repeat"),
                };
                assert_eq!(instruction.kind(), kind, "{context}");
                assert_bitwise_args(instruction.args(), &args, &context);
                assert_eq!(instruction.targets(), targets.as_slice(), "{context}");
                assert_eq!(
                    instruction.tag_bytes(),
                    expected_tag(tag.as_ref()),
                    "{context}"
                );
            }
            (DemItem::RepeatBlock(_), _) => panic!("{context}: parsed repeat for instruction"),
        }
    }
}

fn schema_items(items: &[GenDem]) -> Vec<SchemaItem> {
    items
        .iter()
        .map(|item| {
            let instruction = |kind: u8, args: Vec<f64>, targets, tag: &Option<Vec<u8>>| {
                SchemaItem::Instruction {
                    head: SchemaHead::DemKind(kind),
                    args,
                    targets,
                    tag: expected_tag(tag.as_ref()).map(<[u8]>::to_vec),
                }
            };
            match item {
                GenDem::Error {
                    tag,
                    probability,
                    groups,
                    ..
                } => {
                    let mut targets = Vec::new();
                    for (group_index, group) in groups.iter().enumerate() {
                        if group_index > 0 {
                            targets.push(SchemaTarget::Separator);
                        }
                        targets.extend(group.iter().map(|target| match target {
                            GenTarget::Detector(id) => SchemaTarget::Detector(*id),
                            GenTarget::Observable(id) => SchemaTarget::Observable(u64::from(*id)),
                        }));
                    }
                    instruction(1, vec![*probability], targets, tag)
                }
                GenDem::Detector {
                    tag,
                    coordinates,
                    detector,
                    ..
                } => instruction(
                    2,
                    coordinates.clone(),
                    vec![SchemaTarget::Detector(*detector)],
                    tag,
                ),
                GenDem::Observable {
                    tag, observable, ..
                } => instruction(
                    3,
                    Vec::new(),
                    vec![SchemaTarget::Observable(u64::from(*observable))],
                    tag,
                ),
                GenDem::Shift {
                    tag,
                    coordinates,
                    shift,
                    ..
                } => instruction(
                    4,
                    coordinates.clone(),
                    vec![SchemaTarget::Numeric(*shift)],
                    tag,
                ),
                GenDem::Repeat {
                    tag, count, body, ..
                } => SchemaItem::Repeat {
                    count: *count,
                    tag: expected_tag(tag.as_ref()).map(<[u8]>::to_vec),
                    body: schema_items(body),
                },
            }
        })
        .collect()
}

fn config(seed: u64, cases: u32) -> Config {
    Config {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config(0xde5_7e47_0001, 768))]

    #[test]
    fn generated_dem_text_parses_exactly_and_prints_to_a_fixed_point(
        items in dem_tree(),
        layout in layout(),
    ) {
        let input = render(&items, layout);
        let shown = String::from_utf8_lossy(&input).into_owned();
        let parsed = DetectorErrorModel::from_dem_bytes(&input)
            .unwrap_or_else(|error| panic!("generated DEM failed to parse: {error}\n{shown}"));
        assert_model_matches(&parsed, &items, "parsed input");

        let printed = parsed.to_dem_bytes();
        let printed_shown = String::from_utf8_lossy(&printed).into_owned();
        let reparsed = DetectorErrorModel::from_dem_bytes(&printed).unwrap_or_else(|error| {
            panic!("canonical DEM failed to reparse: {error}\ninput:\n{shown}\nprinted:\n{printed_shown}")
        });
        assert_model_matches(&reparsed, &items, "reparsed canonical");
        prop_assert!(reparsed == parsed, "reparsed != parsed\n{printed_shown}");
        prop_assert_eq!(reparsed.to_dem_bytes(), printed.clone(), "printer is not a fixed point");
        prop_assert_eq!(
            parsed.fingerprint().digest(),
            schema_digest(DEM_DIALECT, &schema_items(&items)),
            "fingerprint differs from the schema-v1 encoding of the generated tree"
        );
        prop_assert_eq!(reparsed.fingerprint(), parsed.fingerprint());

        if let Ok(text) = std::str::from_utf8(&input) {
            let from_str = DetectorErrorModel::from_dem_str(text)
                .unwrap_or_else(|error| panic!("UTF-8 DEM failed through from_dem_str: {error}"));
            prop_assert!(from_str == parsed, "str and byte parsers disagree\n{shown}");
            prop_assert_eq!(from_str.to_dem_string().into_bytes(), printed);
        }
    }

    #[test]
    fn api_built_dem_matches_its_parsed_canonical_text(
        items in dem_tree(),
    ) {
        let Some(built) = build_text_tagged(&items) else {
            return Ok(());
        };
        let printed = built.to_dem_string();
        let reparsed = DetectorErrorModel::from_dem_str(&printed)
            .unwrap_or_else(|error| panic!("API-built DEM failed to reparse: {error}\n{printed}"));
        assert_model_matches(&reparsed, &items, "API-built canonical");
        prop_assert!(reparsed == built, "API-built model changed through text\n{printed}");
        prop_assert_eq!(reparsed.to_dem_string(), printed);
    }
}

/// Builds the generated tree through the public construction API, or returns `None` when a tag
/// is not UTF-8 (the string API cannot carry opaque tag bytes).
fn build_text_tagged(items: &[GenDem]) -> Option<DetectorErrorModel> {
    let text_tag = |tag: &Option<Vec<u8>>| -> Option<Option<String>> {
        match tag {
            None => Some(None),
            Some(bytes) => String::from_utf8(bytes.clone()).ok().map(Some),
        }
    };
    let mut model = DetectorErrorModel::new();
    for item in items {
        match item {
            GenDem::Repeat {
                tag, count, body, ..
            } => model.push_repeat_block(DemRepeatBlock::new(
                DemRepeatCount::new(*count),
                build_text_tagged(body)?,
                text_tag(tag)?,
            )),
            GenDem::Error {
                tag,
                probability,
                groups,
                ..
            } => model.push_instruction(
                stab_model::DemInstruction::error(
                    stab_model::Probability::try_new(*probability).expect("valid probability"),
                    expected_targets(groups),
                    text_tag(tag)?,
                )
                .expect("valid error"),
            ),
            GenDem::Detector {
                tag,
                coordinates,
                detector,
                ..
            } => model.push_instruction(
                stab_model::DemInstruction::detector(
                    coordinates.clone(),
                    DemTarget::relative_detector(*detector).expect("valid id"),
                    text_tag(tag)?,
                )
                .expect("valid detector"),
            ),
            GenDem::Observable {
                tag, observable, ..
            } => model.push_instruction(
                stab_model::DemInstruction::logical_observable(
                    DemTarget::logical_observable(u64::from(*observable)).expect("valid id"),
                    text_tag(tag)?,
                )
                .expect("valid observable"),
            ),
            GenDem::Shift {
                tag,
                coordinates,
                shift,
                ..
            } => model.push_instruction(
                stab_model::DemInstruction::shift_detectors(
                    coordinates.clone(),
                    *shift,
                    text_tag(tag)?,
                )
                .expect("valid shift"),
            ),
        }
    }
    Some(model)
}
