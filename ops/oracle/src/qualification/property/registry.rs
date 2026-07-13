use std::time::Duration;

use sha2::{Digest as _, Sha256};

use super::persistence::parse_persistence;
#[cfg(test)]
use super::{FailureReason, PropertyCaseIndex};
use super::{
    MAX_TARGET_PERSISTENCE_BYTES, MinimizedPropertyFailure, PropertyCase, PropertyPlan,
    PropertyRunError, PropertySeed,
};
use crate::qualification::model::{
    PropertyExecutionMode, PropertyExecutionPlan, PropertyPersistencePolicy,
};
use crate::{CapturedOutput, ProcessOutput};

pub(crate) const PASS_TARGET_ID: &str = "cq1-property-worker-pass";
const FAILURE_TARGET_ID: &str = "cq1-property-worker-failure";
pub(crate) const TIMEOUT_TARGET_ID: &str = "cq1-property-worker-timeout";
#[cfg(test)]
pub(crate) const LARGE_FAILURE_TARGET_ID: &str = "cq1-property-worker-large-failure";
#[cfg(test)]
const LARGE_FAILURE_BYTES: usize = (1024 * 1024) + 64 * 1024;
const TARGET_PERSISTENCE_MAGIC: &str = "STAB-CQ1-PROPERTY-TARGET-1";
const TARGET_PAYLOAD_MARKER: &[u8] = b"\npayload-follows\n\n";

#[derive(Clone, Copy)]
enum RegisteredTarget {
    Pass,
    Failure,
    #[cfg(test)]
    LargeFailure,
    Timeout,
}

struct RegisteredTargetContract {
    generator_domain: &'static str,
    seed: u64,
    cases: usize,
    maximum_generated_bytes: usize,
}

pub(crate) fn is_registered_target(id: &str) -> bool {
    registered_target(id).is_some()
}

pub(crate) fn execution_plan(id: &str) -> Option<PropertyExecutionPlan> {
    let target = registered_target(id)?;
    let contract = contract(target);
    Some(PropertyExecutionPlan {
        generator_domain: contract.generator_domain.to_string(),
        maximum_generated_bytes: contract.maximum_generated_bytes,
        seeds: vec![contract.seed],
        case_count: u32::try_from(contract.cases).ok()?,
        corpus_path: None,
        corpus_sha256: None,
        persistence_policy: PropertyPersistencePolicy::PersistMinimizedRegression,
        execution_mode: PropertyExecutionMode::QualificationWorkerSubprocess,
    })
}

pub(crate) fn execution_plan_digest(id: &str) -> Result<String, String> {
    let plan =
        execution_plan(id).ok_or_else(|| format!("property target {id:?} is not registered"))?;
    let encoded = serde_json::to_vec(&(id, plan)).map_err(|source| source.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(b"stab-cq1/property-plan/v1\0");
    hasher.update(encoded);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn execution_plan_matches(id: &str, actual: &PropertyExecutionPlan) -> bool {
    execution_plan(id).as_ref() == Some(actual)
}

pub(crate) fn run_registered_worker(
    id: &str,
    expected_plan_digest: &str,
) -> Result<ProcessOutput, String> {
    let target =
        registered_target(id).ok_or_else(|| format!("property target {id:?} is not registered"))?;
    let actual_plan_digest = execution_plan_digest(id)?;
    if actual_plan_digest != expected_plan_digest {
        return Err(format!(
            "property target {id:?} plan digest does not match its registered contract"
        ));
    }
    if matches!(target, RegisteredTarget::Timeout) {
        std::thread::sleep(Duration::from_secs(30));
    }
    #[cfg(test)]
    if matches!(target, RegisteredTarget::LargeFailure) {
        return Ok(large_failure_output(id));
    }
    let plan = plan(target)?;
    match plan.run(
        |case| generate(target, case),
        |bytes| predicate(target, bytes),
    ) {
        Ok(summary) => Ok(process_output(
            Some(0),
            format!(
                "property target {id} passed {} cases across {} seeds\n",
                summary.evaluated_cases(),
                summary.seed_count()
            )
            .into_bytes(),
            Vec::new(),
        )),
        Err(PropertyRunError::Failure(failure)) => Ok(failure_output(id, &failure)),
        Err(source) => Ok(process_output(
            Some(1),
            Vec::new(),
            format!("property target {id} failed: {source}\n").into_bytes(),
        )),
    }
}

pub(crate) fn replay_registered_failure(id: &str, bytes: &[u8]) -> Result<(), String> {
    let target =
        registered_target(id).ok_or_else(|| format!("property target {id:?} is not registered"))?;
    let (persisted_id, payload) = parse_target_persistence(bytes)?;
    if persisted_id != id {
        return Err(format!(
            "persisted regression belongs to property target {persisted_id:?}, not {id:?}"
        ));
    }
    let failure = parse_persistence(payload).map_err(|source| source.to_string())?;
    match predicate(target, failure.minimized_input()) {
        Err(_) => Ok(()),
        Ok(()) => Err(format!(
            "persisted regression for property target {id:?} no longer reproduces"
        )),
    }
}

fn registered_target(id: &str) -> Option<RegisteredTarget> {
    match id {
        PASS_TARGET_ID => Some(RegisteredTarget::Pass),
        FAILURE_TARGET_ID => Some(RegisteredTarget::Failure),
        #[cfg(test)]
        LARGE_FAILURE_TARGET_ID => Some(RegisteredTarget::LargeFailure),
        TIMEOUT_TARGET_ID => Some(RegisteredTarget::Timeout),
        _ => None,
    }
}

fn plan(target: RegisteredTarget) -> Result<PropertyPlan, String> {
    let contract = contract(target);
    PropertyPlan::try_new(
        PropertySeed::new(contract.seed),
        contract.cases,
        contract.maximum_generated_bytes,
        None,
    )
    .map_err(|source| source.to_string())
}

const fn contract(target: RegisteredTarget) -> RegisteredTargetContract {
    match target {
        RegisteredTarget::Pass => RegisteredTargetContract {
            generator_domain: "CQ1 deterministic registered pass target",
            seed: 0x4d59_5df4_d0f3_3173,
            cases: 8,
            maximum_generated_bytes: 8,
        },
        RegisteredTarget::Failure => RegisteredTargetContract {
            generator_domain: "CQ1 deterministic minimization and replay target",
            seed: 0x94d0_49bb_1331_11eb,
            cases: 1,
            maximum_generated_bytes: 16,
        },
        #[cfg(test)]
        RegisteredTarget::LargeFailure => RegisteredTargetContract {
            generator_domain: "CQ1 dedicated large persistence transport target",
            seed: 0xd6e8_feb8_6659_fd93,
            cases: 1,
            maximum_generated_bytes: LARGE_FAILURE_BYTES,
        },
        RegisteredTarget::Timeout => RegisteredTargetContract {
            generator_domain: "CQ1 killable property timeout target",
            seed: 0xbf58_476d_1ce4_e5b9,
            cases: 1,
            maximum_generated_bytes: 8,
        },
    }
}

fn generate(target: RegisteredTarget, case: PropertyCase) -> Vec<u8> {
    match target {
        RegisteredTarget::Pass | RegisteredTarget::Timeout => {
            case.generated_seed().get().to_le_bytes().to_vec()
        }
        RegisteredTarget::Failure => vec![9, 7, 8, 3],
        #[cfg(test)]
        RegisteredTarget::LargeFailure => large_failure_payload(),
    }
}

fn predicate(target: RegisteredTarget, bytes: &[u8]) -> Result<(), &'static str> {
    match target {
        RegisteredTarget::Pass | RegisteredTarget::Timeout => {
            if bytes.len() == 8 {
                Ok(())
            } else {
                Err("generated seed must occupy exactly eight bytes")
            }
        }
        RegisteredTarget::Failure => {
            if bytes.windows(2).any(|window| window == [7, 8]) {
                Err("contains the frozen failure marker")
            } else {
                Ok(())
            }
        }
        #[cfg(test)]
        RegisteredTarget::LargeFailure => {
            if large_failure_payload_matches(bytes) {
                Err("matches the dedicated large persistence regression")
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
fn large_failure_output(id: &str) -> ProcessOutput {
    let contract = contract(RegisteredTarget::LargeFailure);
    let case = PropertyCase::new(PropertySeed::new(contract.seed), PropertyCaseIndex::new(0));
    let failure = MinimizedPropertyFailure {
        case,
        original_length: LARGE_FAILURE_BYTES,
        reason: FailureReason::from_display(&"matches the dedicated large persistence regression"),
        minimized_input: large_failure_payload(),
    };
    failure_output(id, &failure)
}

#[cfg(test)]
fn large_failure_payload() -> Vec<u8> {
    let mut state = 0xd6e8_feb8_6659_fd93_u64;
    (0..LARGE_FAILURE_BYTES)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
            low_byte(state)
        })
        .collect()
}

#[cfg(test)]
fn large_failure_payload_matches(bytes: &[u8]) -> bool {
    if bytes.len() != LARGE_FAILURE_BYTES {
        return false;
    }
    let mut state = 0xd6e8_feb8_6659_fd93_u64;
    bytes.iter().all(|actual| {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        *actual == low_byte(state)
    })
}

#[cfg(test)]
const fn low_byte(value: u64) -> u8 {
    let [byte, _, _, _, _, _, _, _] = value.to_le_bytes();
    byte
}

fn failure_output(id: &str, failure: &MinimizedPropertyFailure) -> ProcessOutput {
    process_output(
        Some(1),
        render_target_persistence(id, failure),
        format!("property target {id} failed: {failure}\n").into_bytes(),
    )
}

fn render_target_persistence(id: &str, failure: &MinimizedPropertyFailure) -> Vec<u8> {
    let mut bytes =
        format!("{TARGET_PERSISTENCE_MAGIC}\ntarget-id={id}\npayload-follows\n\n").into_bytes();
    bytes.extend_from_slice(failure.render_persistence().as_bytes());
    bytes
}

fn parse_target_persistence(bytes: &[u8]) -> Result<(&str, &[u8]), String> {
    if bytes.len() > MAX_TARGET_PERSISTENCE_BYTES {
        return Err("property target persistence exceeds its bounded protocol size".to_string());
    }
    let marker = bytes
        .windows(TARGET_PAYLOAD_MARKER.len())
        .position(|window| window == TARGET_PAYLOAD_MARKER)
        .ok_or_else(|| "property target persistence is missing its payload marker".to_string())?;
    let header = std::str::from_utf8(
        bytes
            .get(..marker)
            .ok_or_else(|| "property target persistence header is invalid".to_string())?,
    )
    .map_err(|_| "property target persistence header is not UTF-8".to_string())?;
    let mut lines = header.lines();
    if lines.next() != Some(TARGET_PERSISTENCE_MAGIC) {
        return Err("property target persistence has the wrong magic".to_string());
    }
    let id = lines
        .next()
        .and_then(|line| line.strip_prefix("target-id="))
        .ok_or_else(|| "property target persistence has no target id".to_string())?;
    if lines.next().is_some() {
        return Err("property target persistence has unexpected metadata".to_string());
    }
    let payload_start = marker
        .checked_add(TARGET_PAYLOAD_MARKER.len())
        .ok_or_else(|| "property target persistence payload boundary overflowed".to_string())?;
    let payload = bytes
        .get(payload_start..)
        .ok_or_else(|| "property target persistence payload is missing".to_string())?;
    Ok((id, payload))
}

fn process_output(status: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> ProcessOutput {
    ProcessOutput {
        status,
        stdout: CapturedOutput {
            bytes: stdout,
            truncated: false,
        },
        stderr: CapturedOutput {
            bytes: stderr,
            truncated: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_worker_executes_a_real_deterministic_plan() {
        let digest = execution_plan_digest(PASS_TARGET_ID).expect("registered plan digest");
        let output =
            run_registered_worker(PASS_TARGET_ID, &digest).expect("registered passing target");

        assert!(output.success());
        assert!(output.stdout.bytes.starts_with(b"property target"));
        assert!(output.stderr.bytes.is_empty());
    }

    #[test]
    fn registered_worker_minimizes_persists_and_replays_failure() {
        let digest = execution_plan_digest(FAILURE_TARGET_ID).expect("registered plan digest");
        let output = run_registered_worker(FAILURE_TARGET_ID, &digest)
            .expect("registered failing target output");

        assert!(!output.success());
        let (target_id, payload) =
            parse_target_persistence(&output.stdout.bytes).expect("parse target envelope");
        assert_eq!(target_id, FAILURE_TARGET_ID);
        let failure = parse_persistence(payload).expect("parse worker persistence");
        assert_eq!(failure.minimized_input(), [7, 8]);
        replay_registered_failure(FAILURE_TARGET_ID, &output.stdout.bytes)
            .expect("persisted failure should replay");
        assert!(replay_registered_failure(PASS_TARGET_ID, &output.stdout.bytes).is_err());
    }

    #[test]
    fn unknown_registered_target_fails_closed() {
        assert!(!is_registered_target("missing"));
        assert!(run_registered_worker("missing", &"0".repeat(64)).is_err());
    }

    #[test]
    fn registered_worker_rejects_a_stale_plan_digest() {
        assert!(run_registered_worker(PASS_TARGET_ID, &"0".repeat(64)).is_err());
    }
}
