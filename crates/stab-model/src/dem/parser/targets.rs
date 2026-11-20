//! Target-list parsing and the target checks that turn parsed tokens into an instruction payload.

use super::super::instruction::{
    DemPayload, error_probability, validate_coordinates, validate_error_separator_prefix,
    validate_error_targets, validate_no_args,
};
use super::super::{DemArgVec, DemInstructionKind, DemTarget, DemTargetVec};
use super::{
    ParsedDemArguments, argument_token_at, first_character_span, is_target_space, parse_line_error,
    parse_source_uint60, starts_with_target_space, target_context, target_syntax_error,
    target_token_at, trim_target_space_start, unsigned_target_error,
};
use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::line_error;
use crate::parse_limits::ParseAdmission;
use crate::source_text::SourceSlice;
use crate::{ByteSpan, ModelDialect, ModelError, ModelResult, ParseErrorCode, ParseErrorContext};

/// The parsed target tokens of one instruction line.
///
/// A bare number is a target only of `shift_detectors`, where it is the detector offset; the
/// parser keeps the first one beside the real targets so that a misplaced number is reported in
/// the same order and with the same message as before it was split out.
pub(super) struct ParsedDemTargets<'a> {
    /// Every non-numeric token, in order.
    values: DemTargetVec,
    numeric: Option<NumericToken>,
    token_count: usize,
    source: SourceSlice<'a>,
    region: ByteSpan,
}

#[derive(Clone, Copy)]
struct NumericToken {
    /// Position among all target tokens; every earlier token is in `values` at the same index.
    index: usize,
    value: u64,
}

impl ParsedDemTargets<'_> {
    /// Tokens before the first number, which sit in `values` at their token positions.
    fn before_numeric(&self) -> &[DemTarget] {
        let end = self
            .numeric
            .map_or(self.values.len(), |numeric| numeric.index);
        self.values.get(..end).unwrap_or(&self.values)
    }

    /// Checks `args` and the parsed targets against `kind` and builds the instruction payload,
    /// leaving the arguments and targets in place when they are invalid so the error can be
    /// located. The arguments are checked first.
    pub(super) fn take_payload(
        &mut self,
        kind: DemInstructionKind,
        args: &mut DemArgVec,
    ) -> ModelResult<DemPayload> {
        match kind {
            DemInstructionKind::Error => {
                let probability = error_probability(args)?;
                validate_error_separator_prefix(self.before_numeric())?;
                if self.numeric.is_some() {
                    return Err(ModelError::invalid_detector_error_model(
                        "error instructions cannot target raw numbers",
                    ));
                }
                validate_error_targets(&self.values)?;
                Ok(DemPayload::Error {
                    probability,
                    targets: std::mem::take(&mut self.values),
                })
            }
            DemInstructionKind::Detector => {
                validate_coordinates(kind, args)?;
                let detector = self.single_target("detector", DemTarget::detector)?;
                Ok(DemPayload::detector(std::mem::take(args), detector))
            }
            DemInstructionKind::LogicalObservable => {
                validate_no_args(args)?;
                let observable = self.single_target("logical_observable", DemTarget::observable)?;
                Ok(DemPayload::logical_observable(observable))
            }
            DemInstructionKind::ShiftDetectors => {
                validate_coordinates(kind, args)?;
                match (self.token_count, self.numeric) {
                    (1, Some(numeric)) => Ok(DemPayload::ShiftDetectors {
                        coordinates: std::mem::take(args),
                        shift: numeric.value,
                    }),
                    _ => Err(ModelError::invalid_detector_error_model(
                        "shift_detectors requires exactly one numeric target",
                    )),
                }
            }
        }
    }

    fn single_target<T>(
        &self,
        kind: &'static str,
        accepts: fn(DemTarget) -> Option<T>,
    ) -> ModelResult<T> {
        if self.token_count != 1 {
            return Err(ModelError::invalid_detector_error_model(format!(
                "{kind} requires exactly one target"
            )));
        }
        if let Some(numeric) = self.numeric {
            return Err(ModelError::invalid_detector_error_model(format!(
                "{kind} received invalid target {}",
                numeric.value
            )));
        }
        match self.values.as_slice() {
            [target] => accepts(*target).ok_or_else(|| {
                ModelError::invalid_detector_error_model(format!(
                    "{kind} received invalid target {target}"
                ))
            }),
            _ => Err(ModelError::invalid_detector_error_model(format!(
                "{kind} requires exactly one target"
            ))),
        }
    }
}

pub(super) fn parse_dem_targets<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
) -> ModelResult<ParsedDemTargets<'a>> {
    if rest.text().is_empty() {
        return Ok(ParsedDemTargets {
            values: DemTargetVec::new(),
            numeric: None,
            token_count: 0,
            source: SourceSlice::new("", rest.byte_start()),
            region: end_error_span,
        });
    }
    if !starts_with_target_space(rest) {
        return Err(target_syntax_error(
            line_number,
            instruction,
            rest.text(),
            first_character_span(rest),
            "targets must be separated by spacing",
        ));
    }
    let content = trim_target_space_start(rest);
    if content.text().is_empty() {
        return Ok(ParsedDemTargets {
            values: DemTargetVec::new(),
            numeric: None,
            token_count: 0,
            source: content,
            region: content.end_span(),
        });
    }

    let mut values = DemTargetVec::new();
    let mut numeric = None;
    let mut token_count = 0_usize;
    let mut cursor = 0;
    let mut region_start = None;
    let mut region_end = content.byte_start();
    while cursor < content.text().len() {
        while content
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| is_target_space(*byte))
        {
            cursor += 1;
        }
        if cursor == content.text().len() {
            break;
        }
        let start = cursor;
        while content
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| !is_target_space(*byte))
        {
            cursor += 1;
        }
        let token = content
            .slice(start, cursor)
            .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
        match parse_arbitrary_target(line_number, instruction, token)? {
            ParsedToken::Target(target) => {
                admission.admit_target(line_number, token.span())?;
                values.push(target);
            }
            // A number is never retained as a target: a valid one becomes the payload of a
            // `shift_detectors`, held inline in its item, and any other is a validation error.
            ParsedToken::Numeric(value) => {
                numeric.get_or_insert(NumericToken {
                    index: token_count,
                    value,
                });
            }
        }
        token_count += 1;
        region_start.get_or_insert(token.byte_start());
        region_end = token.byte_end();
    }
    let region = region_start
        .and_then(|start| ByteSpan::try_new(start, region_end.saturating_sub(start)))
        .unwrap_or_else(|| content.end_span());
    Ok(ParsedDemTargets {
        values,
        numeric,
        token_count,
        source: content,
        region,
    })
}

enum ParsedToken {
    Target(DemTarget),
    Numeric(u64),
}

fn parse_arbitrary_target(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
) -> ModelResult<ParsedToken> {
    let Some(prefix) = token.text().as_bytes().first().copied() else {
        return Err(target_syntax_error(
            line_number,
            instruction,
            "",
            token.end_span(),
            "invalid DEM target",
        ));
    };
    match prefix {
        b'D' | b'd' => {
            let number = token
                .suffix(1)
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
            let value = parse_source_uint60(number).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    number,
                    "relative detector target",
                    error,
                )
            })?;
            DemTarget::relative_detector(value)
                .map(ParsedToken::Target)
                .map_err(|error| out_of_range_target_error(line_number, instruction, token, &error))
        }
        b'L' | b'l' => {
            let number = token
                .suffix(1)
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
            let value = parse_source_uint60(number).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    number,
                    "logical observable target",
                    error,
                )
            })?;
            DemTarget::logical_observable(value)
                .map(ParsedToken::Target)
                .map_err(|error| out_of_range_target_error(line_number, instruction, token, &error))
        }
        b'^' if token.text().len() == 1 => Ok(ParsedToken::Target(DemTarget::SEPARATOR)),
        b'0'..=b'9' => {
            let value = parse_source_uint60(token).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    token,
                    "numeric DEM target",
                    error,
                )
            })?;
            Ok(ParsedToken::Numeric(value))
        }
        _ => Err(line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            ParseErrorCode::InvalidTargetSyntax,
            "unrecognized DEM target prefix",
            format!(
                "invalid detector error model: invalid DEM target {:?}",
                token.text()
            ),
            first_character_span(token),
            target_context(instruction, token.text()),
        )),
    }
}

fn out_of_range_target_error(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
    error: &ModelError,
) -> ModelError {
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::IntegerOutOfRange,
        super::dem_error_message(error),
        error.to_string(),
        token.span(),
        target_context(instruction, token.text()),
    )
}

pub(super) fn dem_validation_location(
    kind: DemInstructionKind,
    instruction: &str,
    args: &ParsedDemArguments<'_>,
    targets: &ParsedDemTargets<'_>,
) -> (ParseErrorCode, ByteSpan, ParseErrorContext) {
    match kind {
        DemInstructionKind::Error if args.values.len() != 1 => (
            ParseErrorCode::InvalidArgumentCount,
            args.region,
            ParseErrorContext::ArgumentCount {
                dialect: ModelDialect::DetectorErrorModel,
                instruction: bounded_parse_diagnostic_text(instruction),
                expected: "exactly 1",
                actual: args.values.len(),
            },
        ),
        DemInstructionKind::Error
            if args
                .values
                .first()
                .is_some_and(|probability| !(0.0..=1.0).contains(probability)) =>
        {
            let token = argument_token_at(args.source, 0);
            (
                ParseErrorCode::InvalidArgument,
                token.map_or(args.region, SourceSlice::span),
                ParseErrorContext::Argument {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    argument: token.map_or_else(String::new, |token| {
                        bounded_parse_diagnostic_text(token.text())
                    }),
                },
            )
        }
        DemInstructionKind::Error => {
            let checked = targets.before_numeric();
            let ends_line = targets.numeric.is_none();
            let invalid_index = checked
                .iter()
                .enumerate()
                .find_map(|(index, target)| {
                    if !target.is_separator() {
                        return None;
                    }
                    let adjacent = index == 0
                        || (ends_line && index + 1 == checked.len())
                        || checked
                            .get(index - 1)
                            .is_some_and(|previous| previous.is_separator());
                    adjacent.then_some(index)
                })
                .or(targets.numeric.map(|numeric| numeric.index));
            let token = invalid_index.and_then(|index| target_token_at(targets.source, index));
            (
                ParseErrorCode::InvalidTarget,
                token.map_or(targets.region, SourceSlice::span),
                ParseErrorContext::Target {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    target: token.map_or_else(String::new, |token| {
                        bounded_parse_diagnostic_text(token.text())
                    }),
                },
            )
        }
        DemInstructionKind::LogicalObservable if !args.values.is_empty() => (
            ParseErrorCode::InvalidArgumentCount,
            args.region,
            ParseErrorContext::ArgumentCount {
                dialect: ModelDialect::DetectorErrorModel,
                instruction: bounded_parse_diagnostic_text(instruction),
                expected: "exactly 0",
                actual: args.values.len(),
            },
        ),
        DemInstructionKind::Detector
        | DemInstructionKind::LogicalObservable
        | DemInstructionKind::ShiftDetectors
            if targets.token_count != 1 =>
        {
            (
                ParseErrorCode::InvalidTargetCount,
                targets.region,
                ParseErrorContext::TargetCount {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    actual: targets.token_count,
                },
            )
        }
        DemInstructionKind::Detector
            if !targets
                .values
                .first()
                .is_some_and(|target| target.is_detector()) =>
        {
            invalid_dem_target_location(instruction, targets)
        }
        DemInstructionKind::LogicalObservable
            if !targets
                .values
                .first()
                .is_some_and(|target| target.is_observable()) =>
        {
            invalid_dem_target_location(instruction, targets)
        }
        DemInstructionKind::ShiftDetectors if targets.numeric.is_none() => {
            invalid_dem_target_location(instruction, targets)
        }
        _ => (
            ParseErrorCode::InvalidSyntax,
            targets.region,
            super::instruction_context(instruction),
        ),
    }
}

fn invalid_dem_target_location(
    instruction: &str,
    targets: &ParsedDemTargets<'_>,
) -> (ParseErrorCode, ByteSpan, ParseErrorContext) {
    let token = target_token_at(targets.source, 0);
    (
        ParseErrorCode::InvalidTarget,
        token.map_or(targets.region, SourceSlice::span),
        ParseErrorContext::Target {
            dialect: ModelDialect::DetectorErrorModel,
            instruction: bounded_parse_diagnostic_text(instruction),
            target: token.map_or_else(String::new, |token| {
                bounded_parse_diagnostic_text(token.text())
            }),
        },
    )
}
