//! Stim-versus-Stab two-sample distribution comparator for `01` shot records.
//!
//! Exact Stim random streams are not part of the compatibility contract, so a two-sample row runs
//! the same argv through pinned Stim and Stab and asks whether both outputs are plausibly draws
//! from one distribution. Every column's hit rate, plus every declared joint statistic over a
//! column set, is scored with a pooled two-proportion z-test, or with Fisher's exact conditional
//! test when one side's count is constant (0 or every shot), where the z-test has little power.
//! The exact test rejects below the same per-test rate the sigma implies. The plan's false-positive budget is
//! Bonferroni-bounded over all of those tests when the plan is parsed, and the declared column
//! count is enforced on every output line, so the budget checked at validation time is the one the
//! comparison actually spends.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::row::CliArgv;
use super::statistical::{CommonPlanFields, parse_sigma, validate_shots_and_seed};
use crate::statistical_contract::{
    TwoProportionOutcome, two_proportion_outcome, two_sided_normal_tail_bound,
};

/// Upper bound on the number of failing tests spelled out in one mismatch report.
const REPORTED_FAILURE_LIMIT: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TwoSamplePlan {
    sample_count: usize,
    fixed_seed: u64,
    false_positive_rate: f64,
    columns: usize,
    sigma: f64,
    joints: Vec<JointStatistic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JointKind {
    /// At least one column in the set is 1, such as "any detector fired".
    Any,
    /// The XOR of the set is 1, such as a logical observable over raw measurements.
    Parity,
}

impl JointKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Parity => "parity",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JointStatistic {
    kind: JointKind,
    columns: Vec<usize>,
    spec: String,
}

impl JointStatistic {
    /// Evaluates the statistic on one width-checked record.
    fn evaluate(&self, record: &[u8]) -> Result<bool, String> {
        let mut value = false;
        for &column in &self.columns {
            let Some(&byte) = record.get(column) else {
                return Err(format!(
                    "two-sample internal error: {} column {column} is outside a {}-column record",
                    self.label(),
                    record.len()
                ));
            };
            let bit = byte == b'1';
            value = match self.kind {
                JointKind::Any => value || bit,
                JointKind::Parity => value ^ bit,
            };
        }
        Ok(value)
    }

    fn label(&self) -> String {
        format!("joint {}={}", self.kind.as_str(), self.spec)
    }
}

impl TwoSamplePlan {
    pub(super) fn parse(plan: &str) -> Result<Self, String> {
        let mut common = CommonPlanFields::default();
        let mut columns = None;
        let mut sigma = None;
        let mut joint_tokens = Vec::new();
        for token in plan.split(';').map(str::trim) {
            if token.is_empty() || common.parse_token(token)? {
                continue;
            }
            if let Some(value) = token.strip_prefix("columns=") {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid columns {value:?}: {error}"))?;
                if parsed == 0 {
                    return Err("two-sample columns must be positive".to_string());
                }
                if columns.replace(parsed).is_some() {
                    return Err("two-sample plan declares columns more than once".to_string());
                }
                continue;
            }
            if let Some(rest) = token.strip_prefix("two-sample columns within ") {
                let parsed = parse_sigma(token, rest, "two-sample")?;
                if sigma.replace(parsed).is_some() {
                    return Err("two-sample plan declares its sigma more than once".to_string());
                }
                continue;
            }
            if let Some(rest) = token.strip_prefix("joint ") {
                joint_tokens.push(rest);
                continue;
            }
            return Err(format!("unknown two-sample plan token {token:?}"));
        }
        let (sample_count, fixed_seed, false_positive_rate) = common.finish()?;
        let columns =
            columns.ok_or_else(|| "two-sample plan does not declare columns=K".to_string())?;
        let sigma = sigma.ok_or_else(|| {
            "two-sample plan does not declare `two-sample columns within Z sigma`".to_string()
        })?;
        let mut joints = Vec::with_capacity(joint_tokens.len());
        let mut seen = BTreeSet::new();
        for rest in joint_tokens {
            let joint = parse_joint(rest, columns)?;
            if !seen.insert((joint.kind.as_str(), joint.columns.clone())) {
                return Err(format!("duplicate two-sample {}", joint.label()));
            }
            joints.push(joint);
        }
        let plan = Self {
            sample_count,
            fixed_seed,
            false_positive_rate,
            columns,
            sigma,
            joints,
        };
        plan.validate_false_positive_budget()?;
        Ok(plan)
    }

    fn test_count(&self) -> usize {
        self.columns + self.joints.len()
    }

    fn validate_false_positive_budget(&self) -> Result<(), String> {
        let tests = self.test_count();
        let estimated_bound = (tests as f64 * two_sided_normal_tail_bound(self.sigma)).min(1.0);
        if estimated_bound > self.false_positive_rate {
            return Err(format!(
                "false_positive_rate<={} is tighter than the estimated {estimated_bound:.6e} two-sample false-positive bound from {} sigma over {tests} test(s)",
                self.false_positive_rate, self.sigma
            ));
        }
        Ok(())
    }
}

fn parse_joint(rest: &str, columns: usize) -> Result<JointStatistic, String> {
    let (kind, spec) = if let Some(spec) = rest.strip_prefix("any=") {
        (JointKind::Any, spec)
    } else if let Some(spec) = rest.strip_prefix("parity=") {
        (JointKind::Parity, spec)
    } else {
        return Err(format!(
            "invalid two-sample joint token {rest:?}; expected any=SET or parity=SET"
        ));
    };
    let mut set = BTreeSet::new();
    for item in spec.split(',').map(str::trim) {
        let (first, last) = match item.split_once("..") {
            Some((first, last)) => (parse_column(first)?, parse_column(last)?),
            None => {
                let column = parse_column(item)?;
                (column, column)
            }
        };
        if first > last {
            return Err(format!("two-sample joint range {item:?} is descending"));
        }
        if last >= columns {
            return Err(format!(
                "two-sample joint column {last} is outside the declared {columns} column(s)"
            ));
        }
        for column in first..=last {
            if !set.insert(column) {
                return Err(format!(
                    "two-sample joint set {spec:?} repeats column {column}"
                ));
            }
        }
    }
    Ok(JointStatistic {
        kind,
        columns: set.into_iter().collect(),
        spec: spec.to_string(),
    })
}

fn parse_column(text: &str) -> Result<usize, String> {
    text.parse::<usize>()
        .map_err(|error| format!("invalid two-sample joint column {text:?}: {error}"))
}

impl TwoSamplePlan {
    /// Checks that the argv requests `01` records and the plan's sample count and seed.
    pub(super) fn validate_argv(&self, argv: &CliArgv) -> Option<String> {
        if let Some(format) = argv.option_value("--out_format")
            && format != "01"
        {
            return Some(format!(
                "two-sample rows parse 01 records, but argv requests --out_format {format}"
            ));
        }
        validate_shots_and_seed(self.sample_count, self.fixed_seed, argv)
    }
}

/// Hit counts for one side: one entry per column followed by one entry per joint statistic.
struct SampleCounts {
    hits: Vec<u64>,
}

impl SampleCounts {
    fn tally(plan: &TwoSamplePlan, side: &str, bytes: &[u8]) -> Result<Self, String> {
        let Some(body) = bytes.strip_suffix(b"\n") else {
            return Err(if bytes.is_empty() {
                format!("{side} stdout has no shots")
            } else {
                format!("{side} stdout does not end with a newline")
            });
        };
        let mut hits = vec![0u64; plan.test_count()];
        let mut shots = 0usize;
        for (line_index, record) in body.split(|&byte| byte == b'\n').enumerate() {
            if record.len() != plan.columns {
                return Err(format!(
                    "{side} shot {line_index} has {} column(s), plan declares columns={}",
                    record.len(),
                    plan.columns
                ));
            }
            if let Some(byte) = record.iter().find(|&&byte| byte != b'0' && byte != b'1') {
                return Err(format!(
                    "{side} shot {line_index} contains non-01 byte 0x{byte:02x}"
                ));
            }
            let (column_hits, joint_hits) = hits.split_at_mut(plan.columns);
            for (count, &byte) in column_hits.iter_mut().zip(record) {
                *count += u64::from(byte == b'1');
            }
            for (count, joint) in joint_hits.iter_mut().zip(&plan.joints) {
                *count += u64::from(joint.evaluate(record)?);
            }
            shots += 1;
        }
        if shots != plan.sample_count {
            return Err(format!(
                "{side} stdout has {shots} shot(s), plan declares sample_count={}",
                plan.sample_count
            ));
        }
        Ok(Self { hits })
    }
}

/// Compares Stim and Stab `01` stdout under a two-sample plan, returning a failure reason.
pub(super) fn compare_two_sample_plan(
    plan: &TwoSamplePlan,
    stim: &[u8],
    stab: &[u8],
) -> Option<String> {
    let per_test_alpha = two_sided_normal_tail_bound(plan.sigma);
    let stim = match SampleCounts::tally(plan, "Stim", stim) {
        Ok(counts) => counts,
        Err(reason) => return Some(reason),
    };
    let stab = match SampleCounts::tally(plan, "Stab", stab) {
        Ok(counts) => counts,
        Err(reason) => return Some(reason),
    };
    let Ok(shots) = u64::try_from(plan.sample_count) else {
        return Some("two-sample sample count exceeds the supported u64 range".to_string());
    };
    let mut failures = Vec::new();
    for (index, (&stim_hits, &stab_hits)) in stim.hits.iter().zip(&stab.hits).enumerate() {
        let label = match index.checked_sub(plan.columns) {
            None => format!("column {index}"),
            Some(joint) => match plan.joints.get(joint) {
                Some(joint) => joint.label(),
                None => {
                    return Some(format!(
                        "two-sample internal error: test {index} has no column or joint"
                    ));
                }
            },
        };
        let Some(outcome) = two_proportion_outcome(stim_hits, stab_hits, shots) else {
            return Some(format!(
                "two-sample internal error: {label} counts {stim_hits}/{stab_hits} exceed {shots} shots"
            ));
        };
        if !outcome.rejects(plan.sigma, per_test_alpha) {
            continue;
        }
        let detail = match outcome {
            TwoProportionOutcome::Identical => continue,
            TwoProportionOutcome::DeterministicDisagreement => {
                "deterministic disagreement".to_string()
            }
            TwoProportionOutcome::Scored { z } => format!("z={z:+.2}"),
            TwoProportionOutcome::Exact { p_value } => format!(
                "one side constant, exact conditional p={p_value:.3e} < {per_test_alpha:.3e}"
            ),
        };
        failures.push(format!(
            "{label}: Stim {stim_hits}/{shots} ({:.5}) vs Stab {stab_hits}/{shots} ({:.5}), {detail}",
            stim_hits as f64 / shots as f64,
            stab_hits as f64 / shots as f64,
        ));
    }
    if failures.is_empty() {
        return None;
    }
    let mut reason = format!(
        "{} of {} two-sample test(s) reject equality at {} sigma",
        failures.len(),
        plan.test_count(),
        plan.sigma
    );
    for failure in failures.iter().take(REPORTED_FAILURE_LIMIT) {
        let _ = write!(reason, "\n  {failure}");
    }
    if failures.len() > REPORTED_FAILURE_LIMIT {
        let _ = write!(
            reason,
            "\n  ... {} more",
            failures.len() - REPORTED_FAILURE_LIMIT
        );
    }
    Some(reason)
}

#[cfg(test)]
mod tests;
