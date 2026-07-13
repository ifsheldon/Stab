#![allow(
    dead_code,
    reason = "CQ1 property runner is tested before planned property target ids become executable in CQ2-CQ5"
)]
#![cfg_attr(
    test,
    allow(
        clippy::panic,
        reason = "property-runner tests use exhaustive match panics for compact failure diagnostics"
    )
)]

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::num::NonZeroU32;

mod persistence;
mod registry;

#[cfg(test)]
pub(crate) use registry::{LARGE_FAILURE_TARGET_ID, TIMEOUT_TARGET_ID};
pub(crate) use registry::{
    PASS_TARGET_ID, execution_plan as registered_execution_plan,
    execution_plan_digest as registered_execution_plan_digest,
    execution_plan_matches as registered_execution_plan_matches, is_registered_target,
    replay_registered_failure, run_registered_worker,
};

pub(crate) const MAX_PROPERTY_CASES_PER_SEED: usize = 1_000_000;
pub(crate) const MAX_PROPERTY_SEED_PANEL_LEN: usize = 64;
pub(crate) const MAX_GENERATED_CASE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_FAILURE_REASON_BYTES: usize = 1_024;
pub(crate) const MAX_FAILURE_DIAGNOSTIC_BYTES: usize = MAX_FAILURE_REASON_BYTES + 256;
const MAX_PERSISTENCE_METADATA_BYTES: usize = (2 * MAX_FAILURE_REASON_BYTES) + 512;
pub(crate) const MAX_PERSISTENCE_BYTES: usize =
    MAX_GENERATED_CASE_BYTES + MAX_PERSISTENCE_METADATA_BYTES;
pub(crate) const MAX_TARGET_PERSISTENCE_BYTES: usize = MAX_PERSISTENCE_BYTES + 256;

const CASE_SEED_INCREMENT: u64 = 0x9e37_79b9_7f4a_7c15;
const CASE_SEED_MIX_1: u64 = 0xbf58_476d_1ce4_e5b9;
const CASE_SEED_MIX_2: u64 = 0x94d0_49bb_1331_11eb;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PropertySeed(u64);

impl PropertySeed {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    fn expand(self, case_index: PropertyCaseIndex) -> Self {
        let offset = u64::from(case_index.get()).wrapping_add(1);
        let mut value = self
            .0
            .wrapping_add(CASE_SEED_INCREMENT.wrapping_mul(offset));
        value = (value ^ (value >> 30)).wrapping_mul(CASE_SEED_MIX_1);
        value = (value ^ (value >> 27)).wrapping_mul(CASE_SEED_MIX_2);
        Self(value ^ (value >> 31))
    }
}

impl fmt::Display for PropertySeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyCaseCount(NonZeroU32);

impl PropertyCaseCount {
    fn try_new(value: usize) -> Result<Self, PropertyPlanError> {
        if value == 0 {
            return Err(PropertyPlanError::ZeroCaseCount);
        }
        if value > MAX_PROPERTY_CASES_PER_SEED {
            return Err(PropertyPlanError::TooManyCases {
                actual: value,
                maximum: MAX_PROPERTY_CASES_PER_SEED,
            });
        }
        let converted = u32::try_from(value).map_err(|_| PropertyPlanError::TooManyCases {
            actual: value,
            maximum: MAX_PROPERTY_CASES_PER_SEED,
        })?;
        let Some(nonzero) = NonZeroU32::new(converted) else {
            return Err(PropertyPlanError::ZeroCaseCount);
        };
        Ok(Self(nonzero))
    }

    pub(crate) const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedBytesLimit(usize);

impl GeneratedBytesLimit {
    fn try_new(value: usize) -> Result<Self, PropertyPlanError> {
        if value > MAX_GENERATED_CASE_BYTES {
            return Err(PropertyPlanError::GeneratedBytesLimitTooLarge {
                actual: value,
                maximum: MAX_GENERATED_CASE_BYTES,
            });
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PropertyPlan {
    primary_seed: PropertySeed,
    case_count: PropertyCaseCount,
    maximum_generated_bytes: GeneratedBytesLimit,
    seed_panel: Box<[PropertySeed]>,
}

impl PropertyPlan {
    pub(crate) fn try_new(
        primary_seed: PropertySeed,
        case_count: usize,
        maximum_generated_bytes: usize,
        seed_panel: Option<Vec<PropertySeed>>,
    ) -> Result<Self, PropertyPlanError> {
        let case_count = PropertyCaseCount::try_new(case_count)?;
        let maximum_generated_bytes = GeneratedBytesLimit::try_new(maximum_generated_bytes)?;
        let seed_panel = seed_panel.unwrap_or_default();
        if seed_panel.len() > MAX_PROPERTY_SEED_PANEL_LEN {
            return Err(PropertyPlanError::SeedPanelTooLarge {
                actual: seed_panel.len(),
                maximum: MAX_PROPERTY_SEED_PANEL_LEN,
            });
        }

        let mut unique = BTreeSet::new();
        unique.insert(primary_seed);
        for seed in &seed_panel {
            if !unique.insert(*seed) {
                return Err(PropertyPlanError::DuplicateSeed(*seed));
            }
        }

        Ok(Self {
            primary_seed,
            case_count,
            maximum_generated_bytes,
            seed_panel: seed_panel.into_boxed_slice(),
        })
    }

    pub(crate) const fn primary_seed(&self) -> PropertySeed {
        self.primary_seed
    }

    pub(crate) const fn case_count(&self) -> PropertyCaseCount {
        self.case_count
    }

    pub(crate) const fn maximum_generated_bytes(&self) -> GeneratedBytesLimit {
        self.maximum_generated_bytes
    }

    pub(crate) fn seed_panel(&self) -> &[PropertySeed] {
        &self.seed_panel
    }

    pub(crate) fn seeds(&self) -> impl Iterator<Item = PropertySeed> + '_ {
        std::iter::once(self.primary_seed).chain(self.seed_panel.iter().copied())
    }

    pub(crate) fn run<G, P, Reason>(
        &self,
        mut generator: G,
        mut predicate: P,
    ) -> Result<PropertyRunSummary, PropertyRunError>
    where
        G: FnMut(PropertyCase) -> Vec<u8>,
        P: FnMut(&[u8]) -> Result<(), Reason>,
        Reason: fmt::Display,
    {
        let mut evaluated_cases = 0_u64;
        for seed in self.seeds() {
            for raw_case_index in 0..self.case_count.get() {
                let case = PropertyCase::new(seed, PropertyCaseIndex(raw_case_index));
                let generated = generator(case);
                self.check_generated_size(case, generated.len())?;
                evaluated_cases += 1;

                let first_reason = match predicate(&generated) {
                    Ok(()) => continue,
                    Err(reason) => FailureReason::from_display(&reason),
                };

                let reproduced = generator(case);
                self.check_generated_size(case, reproduced.len())?;
                if reproduced != generated {
                    return Err(PropertyRunError::GeneratorDidNotReproduce {
                        case,
                        first_length: generated.len(),
                        reproduced_length: reproduced.len(),
                    });
                }
                if predicate(&reproduced).is_ok() {
                    return Err(PropertyRunError::FailureDidNotReproduce {
                        case,
                        reason: first_reason,
                    });
                }

                let original_length = generated.len();
                let (minimized_input, reason) =
                    shrink_failure(case, generated, first_reason, &mut predicate)?;
                return Err(PropertyRunError::Failure(MinimizedPropertyFailure {
                    case,
                    original_length,
                    reason,
                    minimized_input,
                }));
            }
        }

        Ok(PropertyRunSummary {
            evaluated_cases,
            seed_count: 1 + self.seed_panel.len(),
        })
    }

    fn check_generated_size(
        &self,
        case: PropertyCase,
        actual: usize,
    ) -> Result<(), PropertyRunError> {
        let maximum = self.maximum_generated_bytes.get();
        if actual > maximum {
            Err(PropertyRunError::GeneratedCaseTooLarge {
                case,
                actual,
                maximum,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PropertyPlanError {
    ZeroCaseCount,
    TooManyCases { actual: usize, maximum: usize },
    GeneratedBytesLimitTooLarge { actual: usize, maximum: usize },
    SeedPanelTooLarge { actual: usize, maximum: usize },
    DuplicateSeed(PropertySeed),
}

impl fmt::Display for PropertyPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCaseCount => write!(formatter, "property plan case count must be nonzero"),
            Self::TooManyCases { actual, maximum } => write!(
                formatter,
                "property plan has {actual} cases per seed, exceeding the limit of {maximum}"
            ),
            Self::GeneratedBytesLimitTooLarge { actual, maximum } => write!(
                formatter,
                "property plan generated-byte limit {actual} exceeds the runner limit of {maximum}"
            ),
            Self::SeedPanelTooLarge { actual, maximum } => write!(
                formatter,
                "property plan seed panel has {actual} seeds, exceeding the limit of {maximum}"
            ),
            Self::DuplicateSeed(seed) => {
                write!(formatter, "property plan repeats seed {seed}")
            }
        }
    }
}

impl std::error::Error for PropertyPlanError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyCaseIndex(u32);

impl PropertyCaseIndex {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyCase {
    seed: PropertySeed,
    case_index: PropertyCaseIndex,
    generated_seed: PropertySeed,
}

impl PropertyCase {
    fn new(seed: PropertySeed, case_index: PropertyCaseIndex) -> Self {
        Self {
            seed,
            case_index,
            generated_seed: seed.expand(case_index),
        }
    }

    pub(crate) const fn seed(self) -> PropertySeed {
        self.seed
    }

    pub(crate) const fn case_index(self) -> PropertyCaseIndex {
        self.case_index
    }

    pub(crate) const fn generated_seed(self) -> PropertySeed {
        self.generated_seed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PropertyRunSummary {
    evaluated_cases: u64,
    seed_count: usize,
}

impl PropertyRunSummary {
    pub(crate) const fn evaluated_cases(self) -> u64 {
        self.evaluated_cases
    }

    pub(crate) const fn seed_count(self) -> usize {
        self.seed_count
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FailureReason {
    text: Box<str>,
    truncated: bool,
}

impl FailureReason {
    fn from_display(value: &impl fmt::Display) -> Self {
        let mut writer = BoundedReasonWriter::new();
        if write!(&mut writer, "{value}").is_err() {
            return Self {
                text: "<reason formatting failed>".into(),
                truncated: false,
            };
        }
        Self {
            text: writer.output.into_boxed_str(),
            truncated: writer.truncated,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) const fn was_truncated(&self) -> bool {
        self.truncated
    }
}

impl fmt::Debug for FailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FailureReason")
            .field("text", &self.text)
            .field("truncated", &self.truncated)
            .finish()
    }
}

impl fmt::Display for FailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

struct BoundedReasonWriter {
    output: String,
    truncated: bool,
}

impl BoundedReasonWriter {
    fn new() -> Self {
        Self {
            output: String::with_capacity(MAX_FAILURE_REASON_BYTES),
            truncated: false,
        }
    }
}

impl fmt::Write for BoundedReasonWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining = MAX_FAILURE_REASON_BYTES.saturating_sub(self.output.len());
        if value.len() <= remaining {
            self.output.push_str(value);
            return Ok(());
        }

        let mut end = remaining;
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        if let Some(prefix) = value.get(..end) {
            self.output.push_str(prefix);
        }
        self.truncated = true;
        Ok(())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct MinimizedPropertyFailure {
    case: PropertyCase,
    original_length: usize,
    reason: FailureReason,
    minimized_input: Vec<u8>,
}

impl MinimizedPropertyFailure {
    pub(crate) const fn seed(&self) -> PropertySeed {
        self.case.seed()
    }

    pub(crate) const fn case_index(&self) -> PropertyCaseIndex {
        self.case.case_index()
    }

    pub(crate) const fn generated_seed(&self) -> PropertySeed {
        self.case.generated_seed()
    }

    pub(crate) const fn original_length(&self) -> usize {
        self.original_length
    }

    pub(crate) fn minimized_input(&self) -> &[u8] {
        &self.minimized_input
    }

    pub(crate) fn minimized_length(&self) -> usize {
        self.minimized_input.len()
    }

    pub(crate) const fn reason(&self) -> &FailureReason {
        &self.reason
    }

    pub(crate) fn render_persistence(&self) -> PersistenceBytes {
        let mut header = String::with_capacity(MAX_PERSISTENCE_METADATA_BYTES);
        header.push_str("STAB-CQ1-PROPERTY-1\nseed=");
        header.push_str(&format!("{:016x}", self.seed().get()));
        header.push_str("\ncase-index=");
        header.push_str(&self.case_index().get().to_string());
        header.push_str("\ngenerated-seed=");
        header.push_str(&format!("{:016x}", self.generated_seed().get()));
        header.push_str("\noriginal-bytes=");
        header.push_str(&self.original_length.to_string());
        header.push_str("\nminimized-bytes=");
        header.push_str(&self.minimized_input.len().to_string());
        header.push_str("\nreason-truncated=");
        header.push_str(if self.reason.was_truncated() {
            "true"
        } else {
            "false"
        });
        header.push_str("\nreason-hex=");
        for byte in self.reason.as_str().bytes() {
            header.push(hex_digit(byte >> 4));
            header.push(hex_digit(byte & 0x0f));
        }
        header.push_str("\npayload-follows\n\n");

        let mut rendered = Vec::with_capacity(header.len() + self.minimized_input.len());
        rendered.extend_from_slice(header.as_bytes());
        rendered.extend_from_slice(&self.minimized_input);
        PersistenceBytes(rendered)
    }
}

impl fmt::Debug for MinimizedPropertyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MinimizedPropertyFailure")
            .field("case", &self.case)
            .field("original_length", &self.original_length)
            .field("minimized_length", &self.minimized_input.len())
            .field("reason", &self.reason)
            .finish()
    }
}

impl fmt::Display for MinimizedPropertyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "property failed for seed {} case {} (generated seed {}); shrank {} bytes to {}: {}",
            self.seed(),
            self.case_index().get(),
            self.generated_seed(),
            self.original_length,
            self.minimized_input.len(),
            self.reason
        )
    }
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => '?',
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct PersistenceBytes(Vec<u8>);

impl PersistenceBytes {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.0
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for PersistenceBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistenceBytes")
            .field("length", &self.0.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PropertyRunError {
    GeneratedCaseTooLarge {
        case: PropertyCase,
        actual: usize,
        maximum: usize,
    },
    GeneratorDidNotReproduce {
        case: PropertyCase,
        first_length: usize,
        reproduced_length: usize,
    },
    FailureDidNotReproduce {
        case: PropertyCase,
        reason: FailureReason,
    },
    Failure(MinimizedPropertyFailure),
}

impl fmt::Display for PropertyRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedCaseTooLarge {
                case,
                actual,
                maximum,
            } => write!(
                formatter,
                "property generator produced {actual} bytes for seed {} case {}, exceeding the plan limit of {maximum}",
                case.seed(),
                case.case_index().get()
            ),
            Self::GeneratorDidNotReproduce {
                case,
                first_length,
                reproduced_length,
            } => write!(
                formatter,
                "property generator did not reproduce seed {} case {}: first length {first_length}, reproduced length {reproduced_length}",
                case.seed(),
                case.case_index().get()
            ),
            Self::FailureDidNotReproduce { case, reason } => write!(
                formatter,
                "property failure for seed {} case {} passed its mandatory reproduction rerun; original reason: {reason}",
                case.seed(),
                case.case_index().get()
            ),
            Self::Failure(failure) => failure.fmt(formatter),
        }
    }
}

impl std::error::Error for PropertyRunError {}

fn shrink_failure<P, Reason>(
    case: PropertyCase,
    original: Vec<u8>,
    original_reason: FailureReason,
    predicate: &mut P,
) -> Result<(Vec<u8>, FailureReason), PropertyRunError>
where
    P: FnMut(&[u8]) -> Result<(), Reason>,
    Reason: fmt::Display,
{
    let mut current = original;
    let mut current_reason = original_reason;
    let mut granularity = 2_usize;

    while !current.is_empty() {
        let chunk_length = current.len().div_ceil(granularity);
        let mut start = 0_usize;
        let mut accepted = false;

        while start < current.len() {
            let end = start.saturating_add(chunk_length).min(current.len());
            let candidate = current
                .iter()
                .take(start)
                .chain(current.iter().skip(end))
                .copied()
                .collect::<Vec<_>>();

            if let Some(reason) = consistently_fails(case, &candidate, predicate)? {
                current = candidate;
                current_reason = reason;
                granularity = granularity.saturating_sub(1).max(2);
                accepted = true;
                break;
            }
            start = end;
        }

        if accepted {
            continue;
        }
        if granularity >= current.len() {
            break;
        }
        granularity = granularity.saturating_mul(2).min(current.len());
    }

    Ok((current, current_reason))
}

fn consistently_fails<P, Reason>(
    case: PropertyCase,
    candidate: &[u8],
    predicate: &mut P,
) -> Result<Option<FailureReason>, PropertyRunError>
where
    P: FnMut(&[u8]) -> Result<(), Reason>,
    Reason: fmt::Display,
{
    let first_reason = match predicate(candidate) {
        Ok(()) => return Ok(None),
        Err(reason) => FailureReason::from_display(&reason),
    };
    match predicate(candidate) {
        Ok(()) => Err(PropertyRunError::FailureDidNotReproduce {
            case,
            reason: first_reason,
        }),
        Err(_) => Ok(Some(first_reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plan(seed: u64, cases: usize, maximum_generated_bytes: usize) -> PropertyPlan {
        match PropertyPlan::try_new(
            PropertySeed::new(seed),
            cases,
            maximum_generated_bytes,
            None,
        ) {
            Ok(plan) => plan,
            Err(error) => panic!("valid test plan was rejected: {error}"),
        }
    }

    #[test]
    fn deterministic_case_seed_expansion_is_frozen() {
        let plan = match PropertyPlan::try_new(
            PropertySeed::new(0),
            3,
            0,
            Some(vec![PropertySeed::new(1)]),
        ) {
            Ok(plan) => plan,
            Err(error) => panic!("valid seed plan was rejected: {error}"),
        };
        assert_eq!(plan.primary_seed(), PropertySeed::new(0));
        assert_eq!(plan.case_count().get(), 3);
        assert_eq!(plan.maximum_generated_bytes().get(), 0);
        assert_eq!(plan.seed_panel(), &[PropertySeed::new(1)]);
        let mut cases = Vec::new();
        let result = plan.run(
            |case| {
                cases.push(case);
                Vec::new()
            },
            |_| Ok::<(), &'static str>(()),
        );

        let summary = match result {
            Ok(summary) => summary,
            Err(error) => panic!("passing property run failed: {error}"),
        };
        assert_eq!(summary.evaluated_cases(), 6);
        assert_eq!(summary.seed_count(), 2);
        assert_eq!(
            cases
                .iter()
                .take(3)
                .map(|case| case.generated_seed().get())
                .collect::<Vec<_>>(),
            vec![
                0xe220_a839_7b1d_cdaf,
                0x6e78_9e6a_a1b9_65f4,
                0x06c4_5d18_8009_454f,
            ]
        );
        assert!(
            cases
                .iter()
                .take(3)
                .all(|case| case.seed() == PropertySeed::new(0))
        );
        assert!(
            cases
                .iter()
                .skip(3)
                .all(|case| case.seed() == PropertySeed::new(1))
        );
    }

    #[test]
    fn plan_rejects_zero_cases_and_duplicate_seeds() {
        assert_eq!(
            PropertyPlan::try_new(PropertySeed::new(1), 0, 1, None),
            Err(PropertyPlanError::ZeroCaseCount)
        );
        assert_eq!(
            PropertyPlan::try_new(PropertySeed::new(1), 1, 1, Some(vec![PropertySeed::new(1)])),
            Err(PropertyPlanError::DuplicateSeed(PropertySeed::new(1)))
        );
        assert_eq!(
            PropertyPlan::try_new(
                PropertySeed::new(1),
                1,
                1,
                Some(vec![PropertySeed::new(2), PropertySeed::new(2)])
            ),
            Err(PropertyPlanError::DuplicateSeed(PropertySeed::new(2)))
        );
    }

    #[test]
    fn first_failure_stops_and_reproduces_deterministically() {
        let plan = match PropertyPlan::try_new(
            PropertySeed::new(42),
            3,
            8,
            Some(vec![PropertySeed::new(99)]),
        ) {
            Ok(plan) => plan,
            Err(error) => panic!("valid reproduction plan was rejected: {error}"),
        };

        let run_once = || {
            let mut generated_cases = Vec::new();
            let result = plan.run(
                |case| {
                    generated_cases.push(case);
                    let leading_byte = if case.case_index().get() == 0 { 0 } else { 1 };
                    vec![leading_byte, 7, 9]
                },
                |bytes| {
                    if bytes.first() == Some(&0) {
                        Err("leading zero")
                    } else {
                        Ok(())
                    }
                },
            );
            (result, generated_cases)
        };

        let (first, first_generated) = run_once();
        let (second, second_generated) = run_once();
        assert_eq!(first, second);
        assert_eq!(first_generated, second_generated);
        assert_eq!(first_generated.len(), 2);
        assert!(
            first_generated.iter().all(|case| {
                case.seed() == PropertySeed::new(42) && case.case_index().get() == 0
            })
        );

        let failure = match first {
            Err(PropertyRunError::Failure(failure)) => failure,
            other => panic!("expected minimized first-case failure, got {other:?}"),
        };
        assert_eq!(failure.seed(), PropertySeed::new(42));
        assert_eq!(failure.case_index().get(), 0);
        assert_eq!(failure.original_length(), 3);
        assert_eq!(failure.minimized_input(), &[0]);
        assert_eq!(
            failure.render_persistence(),
            match second {
                Err(PropertyRunError::Failure(failure)) => failure.render_persistence(),
                other => panic!("expected reproduced failure, got {other:?}"),
            }
        );
    }

    #[test]
    fn shrinking_is_deterministic_and_preserves_failure() {
        let plan = test_plan(7, 1, 32);
        let result = plan.run(
            |_| vec![9, 1, 2, 7, 8, 3, 4],
            |bytes| {
                if bytes.windows(2).any(|window| window == [7, 8]) {
                    Err("contains marker")
                } else {
                    Ok(())
                }
            },
        );

        let failure = match result {
            Err(PropertyRunError::Failure(failure)) => failure,
            other => panic!("expected minimized marker failure, got {other:?}"),
        };
        assert_eq!(failure.original_length(), 7);
        assert_eq!(failure.minimized_length(), 2);
        assert_eq!(failure.minimized_input(), &[7, 8]);
        assert!(
            failure
                .minimized_input()
                .windows(2)
                .any(|window| window == [7, 8])
        );
    }

    #[test]
    fn rerun_until_pass_is_rejected() {
        let plan = test_plan(11, 1, 4);
        let mut predicate_calls = 0;
        let result = plan.run(
            |_| vec![1],
            |_| {
                predicate_calls += 1;
                if predicate_calls == 1 {
                    Err("transient failure")
                } else {
                    Ok(())
                }
            },
        );

        assert_eq!(predicate_calls, 2);
        assert!(matches!(
            result,
            Err(PropertyRunError::FailureDidNotReproduce { .. })
        ));
    }

    #[test]
    fn oversized_generated_case_is_rejected_before_predicate() {
        let plan = test_plan(13, 1, 2);
        let mut predicate_called = false;
        let result = plan.run(
            |_| vec![1, 2, 3],
            |_| {
                predicate_called = true;
                Ok::<(), &'static str>(())
            },
        );

        assert!(!predicate_called);
        assert!(matches!(
            result,
            Err(PropertyRunError::GeneratedCaseTooLarge {
                actual: 3,
                maximum: 2,
                ..
            })
        ));
    }

    #[test]
    fn failure_diagnostics_and_persistence_are_bounded() {
        let plan = test_plan(17, 1, 4);
        let oversized_reason = "z".repeat(MAX_FAILURE_REASON_BYTES * 8);
        let result = plan.run(|_| vec![1, 2, 3, 4], |_| Err(oversized_reason.clone()));

        let error = match result {
            Err(error) => error,
            Ok(summary) => panic!("expected failure, got passing summary {summary:?}"),
        };
        assert!(error.to_string().len() <= MAX_FAILURE_DIAGNOSTIC_BYTES);
        let failure = match error {
            PropertyRunError::Failure(failure) => failure,
            other => panic!("expected minimized bounded failure, got {other:?}"),
        };
        assert_eq!(failure.reason().as_str().len(), MAX_FAILURE_REASON_BYTES);
        assert!(failure.reason().was_truncated());

        let rendered = failure.render_persistence();
        assert!(!rendered.is_empty());
        assert!(rendered.len() <= MAX_PERSISTENCE_BYTES);
        assert!(rendered.as_bytes().starts_with(b"STAB-CQ1-PROPERTY-1\n"));
        assert_eq!(rendered.clone().into_vec(), rendered.as_bytes());
    }
}
