use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::worker::WorkerError;
#[cfg(test)]
use super::worker::clifford_string::CLIFFORD_COMPLETE_SPAN;
use super::worker::clifford_string::{
    CLIFFORD_IDENTITY_MARKER, CLIFFORD_NON_IDENTITY_MARKER, CLIFFORD_PUBLIC_CAP,
    CliffordDescriptor, CliffordStringFixture, CliffordWorkloadKind, STIM_GATE_ORDER,
};
use crate::error::BenchError;
use crate::root::RepoRoot;
use crate::source_file::{atomic_write_repo_regular_file, read_repo_regular_file_bounded};

pub(super) const VECTOR_SCHEMA_VERSION: u32 = 1;
pub(super) const VECTOR_PATH: &str = "benchmarks/fixtures/pq2-clifford-string-vectors.json";
const MAX_VECTOR_BYTES: usize = 1 << 20;
const SMALL_WIDTH: u64 = 10_000;
const MEDIUM_WIDTH: u64 = 100_000;
const LARGE_WIDTH: u64 = 1_000_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordVectorFile {
    pub(super) schema_version: u32,
    pub(super) markers: CliffordMarkers,
    pub(super) gate_order: Vec<CliffordGateVector>,
    pub(super) descriptors: Vec<CliffordDescriptorVector>,
    pub(super) tails: Vec<CliffordTailVector>,
    pub(super) requests: Vec<CliffordRequestVector>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordMarkers {
    pub(super) identity: u64,
    pub(super) non_identity: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordGateVector {
    pub(super) code: u8,
    pub(super) name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordDescriptorVector {
    pub(super) id: String,
    pub(super) workload: String,
    pub(super) width: u64,
    pub(super) raw_hex: String,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordTailVector {
    pub(super) width: u64,
    pub(super) tail_length: u64,
    pub(super) final_left_code: u8,
    pub(super) final_right_code: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CliffordRequestResult {
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliffordRequestVector {
    pub(super) id: String,
    pub(super) result: CliffordRequestResult,
    pub(super) workload: String,
    pub(super) measurement_id: String,
    pub(super) iterations: u64,
    pub(super) work_items: u64,
    pub(super) descriptor_hex: String,
    pub(super) input_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) output_fields: Option<[u64; 16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) output_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) expected_rejection_class: Option<String>,
    pub(super) start_barrier_consumed: bool,
}

pub(crate) fn regenerate(root: &RepoRoot, check: bool) -> Result<(), BenchError> {
    let generated =
        generated_file().map_err(|error| BenchError::Qualification(error.to_string()))?;
    let mut bytes = serde_json::to_vec_pretty(&generated)?;
    bytes.push(b'\n');
    let path = root.path.join(VECTOR_PATH);
    if check {
        let checked = read_repo_regular_file_bounded(root, &path, MAX_VECTOR_BYTES)?;
        if checked != bytes {
            return Err(BenchError::Qualification(format!(
                "{VECTOR_PATH} is stale; regenerate the Clifford qualification vectors"
            )));
        }
    } else {
        atomic_write_repo_regular_file(root, &path, &bytes)?;
    }
    Ok(())
}

pub(super) fn checked_file() -> Result<&'static CliffordVectorFile, String> {
    static CHECKED: OnceLock<Result<CliffordVectorFile, String>> = OnceLock::new();
    CHECKED
        .get_or_init(|| {
            let checked: CliffordVectorFile = serde_json::from_slice(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../benchmarks/fixtures/pq2-clifford-string-vectors.json"
            )))
            .map_err(|error| format!("failed to parse {VECTOR_PATH}: {error}"))?;
            let generated = generated_file().map_err(|error| error.to_string())?;
            if checked != generated {
                return Err(format!(
                    "{VECTOR_PATH} differs from the source-owned Clifford contract"
                ));
            }
            Ok(checked)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn generated_file() -> Result<CliffordVectorFile, WorkerError> {
    let descriptors = [
        (
            CliffordWorkloadKind::Identity,
            "identity-small",
            SMALL_WIDTH,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-medium",
            MEDIUM_WIDTH,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-large",
            LARGE_WIDTH,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-maximum",
            CLIFFORD_PUBLIC_CAP,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-small",
            SMALL_WIDTH,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-medium",
            MEDIUM_WIDTH,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-large",
            LARGE_WIDTH,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-maximum",
            CLIFFORD_PUBLIC_CAP,
        ),
    ]
    .into_iter()
    .map(|(kind, id, width)| descriptor_vector(kind, id, width))
    .collect::<Result<Vec<_>, _>>()?;

    let mut requests = Vec::with_capacity(31);
    for (kind, id, width, iterations) in [
        (
            CliffordWorkloadKind::Identity,
            "identity-small-odd",
            SMALL_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-small-even",
            SMALL_WIDTH,
            2,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-medium",
            MEDIUM_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-large",
            LARGE_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::Identity,
            "identity-maximum",
            CLIFFORD_PUBLIC_CAP,
            1,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-small-odd",
            SMALL_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-small-even",
            SMALL_WIDTH,
            2,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-medium",
            MEDIUM_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-large",
            LARGE_WIDTH,
            1,
        ),
        (
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-maximum",
            CLIFFORD_PUBLIC_CAP,
            1,
        ),
    ] {
        requests.push(accepted_request(kind, id, width, iterations)?);
    }

    let identity = CliffordDescriptor::canonical(CliffordWorkloadKind::Identity, SMALL_WIDTH);
    let non_identity =
        CliffordDescriptor::canonical(CliffordWorkloadKind::NonIdentity, SMALL_WIDTH);
    requests.extend([
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-first-over-cap",
            mutate_descriptor(identity, DescriptorMutation::Width(CLIFFORD_PUBLIC_CAP + 1)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "width-limit",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-first-over-cap",
            mutate_descriptor(
                non_identity,
                DescriptorMutation::Width(CLIFFORD_PUBLIC_CAP + 1),
            ),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "width-limit",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-zero-width",
            mutate_descriptor(identity, DescriptorMutation::Width(0)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "zero-width",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-zero-width",
            mutate_descriptor(non_identity, DescriptorMutation::Width(0)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "zero-width",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "unknown-marker",
            mutate_descriptor(identity, DescriptorMutation::Marker(u64::MAX)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "unknown-marker",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-wrong-measurement",
            identity,
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "wrong-measurement",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-wrong-measurement",
            non_identity,
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "wrong-measurement",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-bad-fixture-schema",
            mutate_descriptor(identity, DescriptorMutation::FixtureSchema(2)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "fixture-schema",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-bad-fixture-schema",
            mutate_descriptor(non_identity, DescriptorMutation::FixtureSchema(2)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "fixture-schema",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-bad-gate-count",
            mutate_descriptor(identity, DescriptorMutation::GateCount(23)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "gate-count",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-bad-gate-count",
            mutate_descriptor(non_identity, DescriptorMutation::GateCount(23)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "gate-count",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-bad-cycle-count",
            mutate_descriptor(identity, DescriptorMutation::CycleCount(1)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "cycle-count",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-bad-cycle-count",
            mutate_descriptor(non_identity, DescriptorMutation::CycleCount(22)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "cycle-count",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-bad-cross-product-span",
            mutate_descriptor(identity, DescriptorMutation::CompleteSpan(552)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "cross-product-span",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-bad-cross-product-span",
            mutate_descriptor(non_identity, DescriptorMutation::CompleteSpan(551)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "cross-product-span",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-bad-cap",
            mutate_descriptor(identity, DescriptorMutation::PublicCap(1_048_575)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "public-cap",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-bad-cap",
            mutate_descriptor(non_identity, DescriptorMutation::PublicCap(1_048_575)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "public-cap",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-reserved",
            mutate_descriptor(identity, DescriptorMutation::Reserved(1)),
            CliffordWorkloadKind::Identity.measurement(),
            1,
            "reserved",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-reserved",
            mutate_descriptor(non_identity, DescriptorMutation::Reserved(1)),
            CliffordWorkloadKind::NonIdentity.measurement(),
            1,
            "reserved",
        )?,
        rejected_request(
            CliffordWorkloadKind::Identity,
            "identity-work-overflow",
            identity,
            CliffordWorkloadKind::Identity.measurement(),
            u64::MAX / SMALL_WIDTH + 1,
            "work-overflow",
        )?,
        rejected_request(
            CliffordWorkloadKind::NonIdentity,
            "nonidentity-work-overflow",
            non_identity,
            CliffordWorkloadKind::NonIdentity.measurement(),
            u64::MAX / SMALL_WIDTH + 1,
            "work-overflow",
        )?,
    ]);

    Ok(CliffordVectorFile {
        schema_version: VECTOR_SCHEMA_VERSION,
        markers: CliffordMarkers {
            identity: CLIFFORD_IDENTITY_MARKER,
            non_identity: CLIFFORD_NON_IDENTITY_MARKER,
        },
        gate_order: STIM_GATE_ORDER
            .iter()
            .enumerate()
            .map(|(code, gate)| {
                Ok(CliffordGateVector {
                    code: u8::try_from(code)
                        .map_err(|_| WorkerError::CliffordGateCodeRange(code))?,
                    name: gate.canonical_name().to_string(),
                })
            })
            .collect::<Result<Vec<_>, WorkerError>>()?,
        descriptors,
        tails: vec![
            tail_vector(10_000, 64, 15, 3),
            tail_vector(100_000, 88, 15, 4),
            tail_vector(1_000_000, 328, 15, 14),
            tail_vector(1_048_576, 328, 15, 14),
        ],
        requests,
    })
}

fn descriptor_vector(
    kind: CliffordWorkloadKind,
    id: &str,
    width: u64,
) -> Result<CliffordDescriptorVector, WorkerError> {
    let descriptor = CliffordDescriptor::canonical(kind, width);
    Ok(CliffordDescriptorVector {
        id: id.to_string(),
        workload: kind.workload().to_string(),
        width,
        raw_hex: descriptor.to_string(),
        sha256: descriptor.input_digest()?,
    })
}

fn accepted_request(
    kind: CliffordWorkloadKind,
    id: &str,
    width: u64,
    iterations: u64,
) -> Result<CliffordRequestVector, WorkerError> {
    let descriptor = CliffordDescriptor::canonical(kind, width);
    let mut fixture = CliffordStringFixture::prepare(kind, descriptor, width, iterations)?;
    fixture.reset_execution_state();
    fixture.execute(iterations)?;
    let work_count = iterations
        .checked_mul(width)
        .ok_or(WorkerError::WorkOverflow)?;
    Ok(CliffordRequestVector {
        id: id.to_string(),
        result: CliffordRequestResult::Accepted,
        workload: kind.workload().to_string(),
        measurement_id: kind.measurement().to_string(),
        iterations,
        work_items: width,
        descriptor_hex: descriptor.to_string(),
        input_sha256: descriptor.input_digest()?,
        output_fields: Some(fixture.output_fields(iterations, work_count)?),
        output_sha256: Some(fixture.output_digest(iterations, work_count)?),
        expected_rejection_class: None,
        start_barrier_consumed: true,
    })
}

fn rejected_request(
    kind: CliffordWorkloadKind,
    id: &str,
    descriptor: CliffordDescriptor,
    measurement: &str,
    iterations: u64,
    class: &str,
) -> Result<CliffordRequestVector, WorkerError> {
    Ok(CliffordRequestVector {
        id: id.to_string(),
        result: CliffordRequestResult::Rejected,
        workload: kind.workload().to_string(),
        measurement_id: measurement.to_string(),
        iterations,
        work_items: SMALL_WIDTH,
        descriptor_hex: descriptor.to_string(),
        input_sha256: descriptor.input_digest()?,
        output_fields: None,
        output_sha256: None,
        expected_rejection_class: Some(class.to_string()),
        start_barrier_consumed: false,
    })
}

fn tail_vector(
    width: u64,
    tail_length: u64,
    final_left_code: u8,
    final_right_code: u8,
) -> CliffordTailVector {
    CliffordTailVector {
        width,
        tail_length,
        final_left_code,
        final_right_code,
    }
}

enum DescriptorMutation {
    Width(u64),
    Marker(u64),
    FixtureSchema(u64),
    GateCount(u64),
    CycleCount(u64),
    CompleteSpan(u64),
    PublicCap(u64),
    Reserved(u64),
}

fn mutate_descriptor(
    descriptor: CliffordDescriptor,
    mutation: DescriptorMutation,
) -> CliffordDescriptor {
    let [
        mut width,
        mut marker,
        mut schema,
        mut gate_count,
        mut cycle_count,
        mut complete_span,
        mut public_cap,
        mut reserved,
    ] = descriptor.fields();
    match mutation {
        DescriptorMutation::Width(value) => width = value,
        DescriptorMutation::Marker(value) => marker = value,
        DescriptorMutation::FixtureSchema(value) => schema = value,
        DescriptorMutation::GateCount(value) => gate_count = value,
        DescriptorMutation::CycleCount(value) => cycle_count = value,
        DescriptorMutation::CompleteSpan(value) => complete_span = value,
        DescriptorMutation::PublicCap(value) => public_cap = value,
        DescriptorMutation::Reserved(value) => reserved = value,
    }
    CliffordDescriptor::from_fields([
        width,
        marker,
        schema,
        gate_count,
        cycle_count,
        complete_span,
        public_cap,
        reserved,
    ])
}

pub(super) fn request_for_runtime<'a>(
    file: &'a CliffordVectorFile,
    workload: &str,
    width: u64,
) -> Result<&'a CliffordRequestVector, String> {
    file.requests
        .iter()
        .find(|request| {
            request.result == CliffordRequestResult::Accepted
                && request.workload == workload
                && request.work_items == width
                && request.iterations == 1
        })
        .ok_or_else(|| {
            format!(
                "{VECTOR_PATH} has no one-iteration accepted request for {workload} width {width}"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_vectors_are_complete_and_source_owned() {
        let file = checked_file().expect("checked Clifford vectors");
        assert_eq!(file.schema_version, VECTOR_SCHEMA_VERSION);
        assert_eq!(file.gate_order.len(), 24);
        assert_eq!(file.descriptors.len(), 8);
        assert_eq!(file.tails.len(), 4);
        assert_eq!(file.requests.len(), 31);
        assert_eq!(
            file.requests
                .iter()
                .filter(|request| request.result == CliffordRequestResult::Accepted)
                .count(),
            10
        );
        assert_eq!(
            file.requests
                .iter()
                .filter(|request| request.result == CliffordRequestResult::Rejected)
                .count(),
            21
        );
        assert!(file.requests.iter().take(10).all(|request| {
            request.result == CliffordRequestResult::Accepted
                && request.start_barrier_consumed
                && request.output_fields.is_some()
                && request.output_sha256.is_some()
        }));
        assert!(file.requests.iter().skip(10).all(|request| {
            request.result == CliffordRequestResult::Rejected
                && !request.start_barrier_consumed
                && request.expected_rejection_class.is_some()
        }));
    }

    #[test]
    fn non_identity_runtime_descriptors_cover_complete_cycles() {
        let file = checked_file().expect("checked Clifford vectors");
        for width in [SMALL_WIDTH, MEDIUM_WIDTH, LARGE_WIDTH, CLIFFORD_PUBLIC_CAP] {
            let request =
                request_for_runtime(file, CliffordWorkloadKind::NonIdentity.workload(), width)
                    .expect("non-identity runtime request");
            assert!(width >= CLIFFORD_COMPLETE_SPAN);
            assert_eq!(request.work_items, width);
        }
    }
}
