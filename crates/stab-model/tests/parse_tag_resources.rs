#![allow(
    clippy::expect_used,
    reason = "contract tests extract required failures directly"
)]

//! Circuit and DEM tags are charged against the parse budget before their storage is allocated,
//! and the reported parse hold covers what a parsed tag retains.

use stab_model::{Circuit, DetectorErrorModel, MemoryBudget, ParseLimits, ResourceKind};

fn budget(bytes: usize) -> ParseLimits {
    ParseLimits::default().with_memory_budget(MemoryBudget::from_bytes(bytes as u64))
}

#[test]
fn tag_text_is_charged_before_it_is_allocated() {
    fn rejected_allocations(input: &str) -> allocation_counter::AllocationInfo {
        allocation_counter::measure(|| {
            let error = Circuit::from_stim_str_with_limits(input, budget(input.len() + 1_024))
                .expect_err("budget refuses the tag");
            let resource = error.resource_limit_error().expect("typed rejection");
            assert_eq!(resource.resource(), ResourceKind::RetainedBytes);
            assert_eq!(resource.source_line(), Some(1));
            std::hint::black_box(error);
        })
    }

    for (name, prefix, suffix) in [
        ("instruction tag", "H[", "] 0\n"),
        ("repeat tag", "REPEAT[", "] 2 {\nH 0\n}\n"),
    ] {
        // Escapes shrink the unescaped text, so they are charged at their unescaped length.
        let short = format!("{prefix}{}{suffix}", "a\\B".repeat(1 << 10));
        let long = format!("{prefix}{}{suffix}", "a\\B".repeat(1 << 18));
        let short = rejected_allocations(&short);
        let long = rejected_allocations(&long);
        assert_eq!(long.count_total, short.count_total, "{name}: {long:?}");
        assert_eq!(long.bytes_total, short.bytes_total, "{name}: {long:?}");
    }
}

/// Parses `input` and returns the reported hold, the circuit's own admitted bytes, and the heap
/// bytes the parsed circuit retains.
fn hold_and_retained(input: &[u8], limits: ParseLimits) -> (u64, u64, u64) {
    let mut kept = Vec::with_capacity(1);
    let measured = allocation_counter::measure(|| {
        kept.push(Circuit::from_stim_bytes_admitted(input, limits).expect("fixture parses"));
    });
    let (circuit, hold) = kept.pop().expect("parsed circuit");
    let retained = u64::try_from(measured.bytes_current).expect("parse retains storage");
    (hold, circuit.admitted_bytes(), retained)
}

#[test]
fn opaque_tags_restored_from_bytes_are_charged_before_they_are_built() {
    // Every invalid byte displays as a three-byte U+FFFD, so the display text of these tags is
    // three times their byte length.
    let mut input = b"H[".to_vec();
    input.extend(b"\xff".repeat(4_096));
    input.extend(b"] 0\nH[");
    input.extend(b"\xfe".repeat(4_096));
    input.extend(b"] 1\nH 2\nH 3\n");

    let (hold, admitted, retained) = hold_and_retained(&input, ParseLimits::default());
    assert_eq!(hold, admitted);
    assert!(retained <= hold, "retained {retained} > hold {hold}");

    // The smallest budget that parses the input is set by the last opaque tag's display text,
    // which is refused at that tag's line and span, before it is allocated.
    let (mut low, mut high) = (input.len(), 1_usize << 20);
    while low < high {
        let middle = low + (high - low) / 2;
        if Circuit::from_stim_bytes_with_limits(&input, budget(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let error = Circuit::from_stim_bytes_with_limits(&input, budget(low - 1))
        .expect_err("one byte short is refused");
    let resource = error.resource_limit_error().expect("typed rejection");
    assert_eq!(resource.resource(), ResourceKind::RetainedBytes);
    assert_eq!(resource.source_line(), Some(2), "{resource}");
    let second_tag = 2 + 4_096 + 4 + 2;
    assert_eq!(resource.span().byte_start(), second_tag, "{resource}");
    assert_eq!(resource.span().byte_length(), 4_096, "{resource}");
}

#[test]
fn fused_instructions_restored_from_bytes_hold_exactly_their_admitted_bytes() {
    // The invalid comment sends the input through tag restoration, which fuses only after
    // restoring tags.
    let mut input = b"# \xff\n".to_vec();
    for qubit in 0..2_000 {
        input.extend(format!("H[t] {qubit} {}\n", qubit + 2_000).as_bytes());
    }
    let (hold, admitted, retained) = hold_and_retained(&input, ParseLimits::default());
    assert_eq!(hold, admitted);
    assert!(retained <= hold, "retained {retained} > hold {hold}");
    let circuit = Circuit::from_stim_bytes(&input).expect("fixture parses");
    assert_eq!(circuit.len(), 1);
}

#[test]
fn opaque_dem_tags_restored_from_bytes_are_charged_before_they_are_built() {
    // Every invalid byte displays as a three-byte U+FFFD, so the display text of these tags is
    // three times their byte length.
    let mut input = b"error[".to_vec();
    input.extend(b"\xff".repeat(4_096));
    input.extend(b"](0.125) D0\nerror[");
    input.extend(b"\xfe".repeat(4_096));
    input.extend(b"](0.25) D1\ndetector D2\n");

    let mut kept = Vec::with_capacity(1);
    let measured = allocation_counter::measure(|| {
        kept.push(
            DetectorErrorModel::from_dem_bytes_admitted(&input, ParseLimits::default())
                .expect("fixture parses"),
        );
    });
    let (model, hold) = kept.pop().expect("parsed model");
    let retained = u64::try_from(measured.bytes_current).expect("parse retains storage");
    assert_eq!(hold, model.admitted_bytes());
    assert!(hold > 2 * 3 * 4_096, "hold {hold} omits the display text");
    assert!(retained <= hold, "retained {retained} > hold {hold}");

    // The smallest budget that parses the input is set by the last opaque tag's display text,
    // which is refused at that tag's line and span, before it is allocated.
    let (mut low, mut high) = (input.len(), 1_usize << 20);
    while low < high {
        let middle = low + (high - low) / 2;
        if DetectorErrorModel::from_dem_bytes_with_limits(&input, budget(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let error = DetectorErrorModel::from_dem_bytes_with_limits(&input, budget(low - 1))
        .expect_err("one byte short is refused");
    let resource = error.resource_limit_error().expect("typed rejection");
    assert_eq!(resource.resource(), ResourceKind::RetainedBytes);
    assert_eq!(resource.source_line(), Some(2), "{resource}");
    let second_tag = "error[".len() + 4_096 + "](0.125) D0\n".len() + "error[".len();
    assert_eq!(resource.span().byte_start(), second_tag, "{resource}");
    assert_eq!(resource.span().byte_length(), 4_096, "{resource}");
}

/// Restores the tags of `outer_tag { inner_tag }` and returns the model's admitted bytes with the
/// restored repeat and instruction tag bytes.
fn nested_tags(
    circuit: bool,
    outer_tag: &[u8],
    inner_tag: &[u8],
) -> (u64, Option<Vec<u8>>, Option<Vec<u8>>) {
    let (head, open, inner, close) = if circuit {
        ("REPEAT[", "] 2 {\nH[", "] 0\n", "}\n")
    } else {
        ("repeat[", "] 2 {\nerror[", "](0.125) D0\n", "}\n")
    };
    let mut input = head.as_bytes().to_vec();
    input.extend(outer_tag);
    input.extend(open.as_bytes());
    input.extend(inner_tag);
    input.extend(inner.as_bytes());
    input.extend(close.as_bytes());
    if circuit {
        let (model, hold) = Circuit::from_stim_bytes_admitted(&input, ParseLimits::default())
            .expect("circuit fixture parses");
        assert_eq!(hold, model.admitted_bytes());
        let repeat = model
            .items()
            .first()
            .and_then(|item| item.as_repeat_block())
            .expect("outer item is a repeat block");
        let instruction = repeat
            .body()
            .items()
            .first()
            .and_then(|item| item.as_instruction())
            .expect("inner item is an instruction");
        (
            hold,
            repeat.tag_bytes().map(<[u8]>::to_vec),
            instruction.tag_bytes().map(<[u8]>::to_vec),
        )
    } else {
        let (model, hold) =
            DetectorErrorModel::from_dem_bytes_admitted(&input, ParseLimits::default())
                .expect("DEM fixture parses");
        assert_eq!(hold, model.admitted_bytes());
        let repeat = model
            .items()
            .first()
            .and_then(|item| item.as_repeat_block())
            .expect("outer item is a repeat block");
        let instruction = repeat
            .body()
            .items()
            .first()
            .and_then(|item| item.as_instruction())
            .expect("inner item is an instruction");
        (
            hold,
            repeat.tag_bytes().map(<[u8]>::to_vec),
            instruction.tag_bytes().map(<[u8]>::to_vec),
        )
    }
}

#[test]
fn both_dialects_restore_and_charge_nested_opaque_tags_alike() {
    let outer = b"\xff".repeat(64);
    let inner = b"\xfe".repeat(32);
    let [circuit_overhead, dem_overhead] = [true, false].map(|circuit| {
        let (text_hold, _, _) = nested_tags(circuit, &b"a".repeat(64), &b"b".repeat(32));
        let (opaque_hold, outer_tag, inner_tag) = nested_tags(circuit, &outer, &inner);
        // Each tag is restored in source order onto its own item.
        assert_eq!(outer_tag.as_deref(), Some(&outer[..]), "circuit {circuit}");
        assert_eq!(inner_tag.as_deref(), Some(&inner[..]), "circuit {circuit}");
        let overhead = opaque_hold - text_hold;
        // Every invalid byte displays as a three-byte U+FFFD, on top of the opaque record.
        assert!(
            overhead > 3 * (64 + 32),
            "circuit {circuit}: overhead {overhead}"
        );
        overhead
    });
    assert_eq!(
        circuit_overhead, dem_overhead,
        "dialects charge opaque tags alike"
    );
}
