use super::super::instruction::{DemPayload, validate_dem_args, validate_error_targets};
use super::super::{
    DemArgVec, DemInstruction, DemInstructionKind, DemTag, DemTarget, DemTargetVec,
};
use super::{MAX_DEM_TEXT_INTEGER, MAX_STIM_NUMBER_TOKEN_BYTES};
// Kept out of line: inlined into the parse loop it measurably slows every line.
#[inline(never)]
pub(super) fn parse_canonical_instruction(line: &str) -> Option<DemInstruction> {
    let (kind, rest) = match line.as_bytes().first().copied()? {
        b'e' => (DemInstructionKind::Error, line.strip_prefix("error")?),
        b'd' => (DemInstructionKind::Detector, line.strip_prefix("detector")?),
        b'l' => (
            DemInstructionKind::LogicalObservable,
            line.strip_prefix("logical_observable")?,
        ),
        b's' => (
            DemInstructionKind::ShiftDetectors,
            line.strip_prefix("shift_detectors")?,
        ),
        _ => return None,
    };

    let (tag, rest) = parse_canonical_tag(rest)?;
    let (args, rest) = parse_canonical_args(kind, rest)?;
    let payload = parse_canonical_payload(kind, rest)?;
    validate_dem_args(kind, &args).ok()?;
    if let DemPayload::Error(targets) = &payload {
        validate_error_targets(targets).ok()?;
    }
    Some(DemInstruction::from_validated_parts(args, payload, tag))
}

pub(super) fn parse_canonical_repeat_header(header: &str) -> Option<(u64, Option<DemTag>)> {
    let rest = header.strip_prefix("repeat")?;
    let (tag, rest) = parse_canonical_tag(rest)?;
    let count = parse_canonical_uint60(rest.strip_prefix(' ')?)?;
    Some((count, tag))
}

fn parse_canonical_tag(rest: &str) -> Option<(Option<DemTag>, &str)> {
    let Some(body) = rest.strip_prefix('[') else {
        return Some((None, rest));
    };
    let close = body.as_bytes().iter().position(|byte| *byte == b']')?;
    let raw_tag = body.get(..close)?;
    if raw_tag.is_empty()
        || raw_tag
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'\\' | b'\r' | b'\n'))
    {
        return None;
    }
    Some((DemTag::from_text(raw_tag), body.get(close + 1..)?))
}

fn parse_canonical_args(kind: DemInstructionKind, rest: &str) -> Option<(DemArgVec, &str)> {
    let Some(body) = rest.strip_prefix('(') else {
        return Some((DemArgVec::new(), rest));
    };
    if kind == DemInstructionKind::LogicalObservable {
        return None;
    }
    let close = body.as_bytes().iter().position(|byte| *byte == b')')?;
    let raw_args = body.get(..close)?;
    if raw_args.is_empty() {
        return None;
    }

    let mut args = DemArgVec::new();
    let mut token_start = 0;
    loop {
        let relative_end = raw_args
            .as_bytes()
            .get(token_start..)?
            .iter()
            .position(|byte| *byte == b',')
            .unwrap_or(raw_args.len() - token_start);
        let token_end = token_start + relative_end;
        let token = raw_args.get(token_start..token_end)?;
        if token.is_empty() || token.len() > MAX_STIM_NUMBER_TOKEN_BYTES {
            return None;
        }
        let value = token.parse::<f64>().ok()?;
        if !value.is_finite() {
            return None;
        }
        args.push(value);
        if token_end == raw_args.len() {
            break;
        }
        if kind == DemInstructionKind::Error {
            return None;
        }
        if raw_args.as_bytes().get(token_end + 1) != Some(&b' ') {
            return None;
        }
        token_start = token_end + 2;
        if token_start == raw_args.len() {
            return None;
        }
    }
    Some((args, body.get(close + 1..)?))
}

fn parse_canonical_payload(kind: DemInstructionKind, rest: &str) -> Option<DemPayload> {
    let raw_targets = if rest.is_empty() {
        ""
    } else {
        let raw_targets = rest.strip_prefix(' ')?;
        if raw_targets.is_empty() {
            return None;
        }
        raw_targets
    };
    match kind {
        DemInstructionKind::Error => {
            parse_canonical_error_targets(raw_targets).map(DemPayload::Error)
        }
        // A single-target instruction holds exactly one token, so a space means extra targets.
        _ if raw_targets.is_empty() || raw_targets.contains(' ') => None,
        DemInstructionKind::Detector => {
            DemTarget::relative_detector(parse_prefixed_id(raw_targets, b'D')?)
                .ok()
                .map(DemPayload::Detector)
        }
        DemInstructionKind::LogicalObservable => {
            DemTarget::logical_observable(parse_prefixed_id(raw_targets, b'L')?)
                .ok()
                .map(DemPayload::LogicalObservable)
        }
        DemInstructionKind::ShiftDetectors => {
            parse_canonical_uint60(raw_targets).map(DemPayload::ShiftDetectors)
        }
    }
}

fn parse_canonical_error_targets(raw_targets: &str) -> Option<DemTargetVec> {
    let mut targets = DemTargetVec::new();
    if raw_targets.is_empty() {
        return Some(targets);
    }
    let mut token_start = 0;
    loop {
        let relative_end = raw_targets
            .as_bytes()
            .get(token_start..)?
            .iter()
            .position(|byte| *byte == b' ')
            .unwrap_or(raw_targets.len() - token_start);
        let token_end = token_start + relative_end;
        let token = raw_targets.get(token_start..token_end)?;
        if token.is_empty() {
            return None;
        }
        targets.push(parse_error_target(token)?);
        if token_end == raw_targets.len() {
            break;
        }
        token_start = token_end + 1;
        if token_start == raw_targets.len() {
            return None;
        }
    }
    Some(targets)
}

fn parse_error_target(token: &str) -> Option<DemTarget> {
    if token == "^" {
        return Some(DemTarget::SEPARATOR);
    }
    match token.as_bytes().first().copied()? {
        b'D' => DemTarget::relative_detector(parse_prefixed_id(token, b'D')?).ok(),
        b'L' => DemTarget::logical_observable(parse_prefixed_id(token, b'L')?).ok(),
        _ => None,
    }
}

fn parse_prefixed_id(token: &str, prefix: u8) -> Option<u64> {
    let bytes = token.as_bytes();
    if bytes.first().copied()? != prefix {
        return None;
    }
    parse_canonical_uint60(token.get(1..)?)
}

fn parse_canonical_uint60(token: &str) -> Option<u64> {
    if token.is_empty() || token.len() > 19 {
        return None;
    }
    let mut value = 0_u64;
    for byte in token.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(byte - b'0'))?;
        if value > MAX_DEM_TEXT_INTEGER {
            return None;
        }
    }
    Some(value)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "internal parser equivalence fixtures require both selected paths to succeed"
)]
mod tests {
    use super::*;
    use crate::parse_limits::ParseAdmission;
    use crate::source_text::SourceSlice;
    use crate::{ModelDialect, ParseLimits};

    #[test]
    fn canonical_fast_path_matches_the_diagnostic_parser() {
        for line in [
            "error(0.125) D0",
            "error(1e-3) D0 L1 ^ D2",
            "error[edge](0.25) D0 L1 ^ D2",
            "error(-0)",
            "detector D42",
            "detector[tag-a](0.5, -2, 3) D1000000",
            "detector(0.5, -2, 3) D1000000",
            "logical_observable L100000",
            "shift_detectors 1152921504606846975",
            "shift_detectors(1.5, -2, 3) 1000001",
        ] {
            let fast = parse_canonical_instruction(line).expect("canonical fast path");
            let source = SourceSlice::new(line, 0);
            let mut admission = ParseAdmission::new(
                ModelDialect::DetectorErrorModel,
                line.len(),
                ParseLimits::default(),
            )
            .expect("default admission");
            let generic =
                super::super::parse_dem_instruction(1, source, source.end_span(), &mut admission)
                    .expect("generic parser");
            assert_eq!(fast, generic, "{line}");
        }
    }

    #[test]
    fn ambiguous_or_invalid_text_falls_back_to_the_diagnostic_parser() {
        for line in [
            "ErRoR(0.125) D0",
            "error[edge\\Ctag](0.125) D0",
            "error(0.125)  D0",
            "error(0.125) D0 ",
            "error(0.125) d0",
            "error() D0",
            "error(2) D0",
            "error(0.1, 0.2) D0",
            "error(0.125) ^ D0",
            "error(0.125) D0 ^",
            "error(0.125) D0 ^ ^ D1",
            "detector(1,2) D0",
            "detector D0 D1",
            "logical_observable(1) L0",
            "shift_detectors D0",
            "shift_detectors 1 2",
            "shift_detectors 1152921504606846976",
        ] {
            assert!(
                parse_canonical_instruction(line).is_none(),
                "unexpectedly selected {line:?}"
            );
        }
    }

    #[test]
    fn canonical_repeat_headers_parse_exact_counts_and_tags() {
        for (header, expected_count, expected_tag) in [
            ("repeat 0", 0, None),
            ("repeat 2", 2, None),
            ("repeat[outer] 1000000", 1_000_000, Some("outer")),
            (
                "repeat[边界] 1152921504606846975",
                MAX_DEM_TEXT_INTEGER,
                Some("边界"),
            ),
        ] {
            let (count, tag) = parse_canonical_repeat_header(header).expect("canonical header");
            assert_eq!(count, expected_count, "{header}");
            assert_eq!(tag.as_ref().map(DemTag::as_str), expected_tag, "{header}");
        }
    }

    #[test]
    fn ambiguous_or_invalid_repeat_headers_use_the_diagnostic_parser() {
        for header in [
            "REPEAT 2",
            "repeat  2",
            "repeat\t2",
            "repeat[tag\\Cvalue] 2",
            "repeat[] 2",
            "repeat[tag]2",
            "repeat[tag] 2 ",
            "repeat -1",
            "repeat 1152921504606846976",
        ] {
            assert!(
                parse_canonical_repeat_header(header).is_none(),
                "unexpectedly selected {header:?}"
            );
        }
    }
}
