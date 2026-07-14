use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::classification::{
    classify_public_api_source, classify_upstream_case, classify_upstream_path, default_comparator,
};
use super::extract::{
    CppTestDeclaration, ExtractionError, PYTHON_AST_VERSION, PythonSource,
    extract_cpp_test_cases_bounded, extract_python_test_cases_bounded,
};
use super::model::{
    ApiPath, CaseId, EvidenceCase, EvidenceProvenance, EvidenceSelector, EvidenceState,
    EvidenceStatus, FeatureId, FeatureRecord, Parameterization, PublicApiItem,
    QualificationManifest, RelativeSourcePath, ResourceContract, ResourceKind, SCHEMA_VERSION,
    SelectorKind, SemanticDigest, StableCaseDomain, UpstreamCase, UpstreamOwnership,
    UpstreamProvenance,
};
use super::public_api::{ExtractedPublicApiItem, PublicApiError, generate_rustdoc_inventory};
use crate::RepoRoot;
use crate::blocker_ledger::selector::CargoTestSelector;

mod evidence;
mod property_plan;
mod qualification_cases;

use evidence::{
    behavioral_surface_for_feature, infer_feature_from_oracle_argv, make_planned_evidence_case,
    oracle_behavioral_surface, planned_api_selector, planned_selector,
    semantic_only_resource_contract, statistical_plan_reference,
};

const RUST_TOOLCHAIN: &str = "nightly-2026-06-20";
const CPP_TEST_FILE_COUNT: usize = 103;
const PYTHON_TEST_FILE_COUNT: usize = 91;
const MAX_FILE_LIST_BYTES: usize = 1 << 20;
const MAX_SOURCE_BYTES: usize = 8 << 20;
const MAX_PYTHON_SOURCE_BYTES: usize = 16 << 20;
const MAX_SOURCE_PATH_BYTES: usize = 512;
const MAX_CASES: usize = 8_192;
const MAX_ORACLE_MANIFEST_BYTES: usize = 16 << 20;
const MAX_ORACLE_ROWS: usize = 16_384;
const MAX_BLOCKER_LEDGER_BYTES: usize = 4 << 20;
const MAX_QUALIFICATION_CASES_BYTES: usize = 16 << 20;

#[derive(Debug, Error)]
pub(crate) enum InventoryError {
    #[error("pinned Stim source validation failed: {0}")]
    StimSource(Box<str>),

    #[error("failed to read qualification input {path}: {reason}")]
    Read { path: PathBuf, reason: Box<str> },

    #[error("qualification input {path} is not UTF-8")]
    NonUtf8 { path: PathBuf },

    #[error("qualification source path {0:?} is invalid")]
    InvalidSourcePath(String),

    #[error("qualification source list contains duplicate path {0:?}")]
    DuplicateSourcePath(String),

    #[error("C++ test file list has {actual} paths; expected {expected}")]
    WrongCppFileCount { actual: usize, expected: usize },

    #[error("Python test file list has {actual} paths; expected {expected}")]
    WrongPythonFileCount { actual: usize, expected: usize },

    #[error("failed to list pinned Python tests: {0}")]
    ListPythonTests(Box<str>),

    #[error(
        "listing pinned Python tests failed with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    ListPythonTestsFailed {
        status: String,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error("pinned Python test path list is not UTF-8")]
    NonUtf8PythonList,

    #[error(transparent)]
    Extract(#[from] ExtractionError),

    #[error(transparent)]
    PublicApi(#[from] PublicApiError),

    #[error("qualification inventory has {actual} {kind}; limit is {limit}")]
    TooManyRecords {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },

    #[error("qualification stable-id collision for {0}")]
    StableIdCollision(String),

    #[error(
        "public API item {crate_name}::{path} from {source_path} has no source-owned qualification feature"
    )]
    UnclassifiedPublicApi {
        crate_name: String,
        path: String,
        source_path: PathBuf,
    },

    #[error("failed to serialize qualification semantic payload: {0}")]
    Serialize(serde_json::Error),

    #[error("failed to parse oracle fixture manifest {path}: {source}")]
    ParseOracleManifest { path: PathBuf, source: csv::Error },

    #[error("oracle fixture manifest has more than {limit} rows")]
    TooManyOracleRows { limit: usize },

    #[error("oracle fixture row {id:?} has unknown comparator {comparator:?}")]
    UnknownOracleComparator { id: String, comparator: String },

    #[error("oracle fixture row {id:?} has invalid exact Cargo selector: {reason}")]
    InvalidOracleSelector { id: String, reason: String },

    #[error(
        "implemented oracle fixture row {id:?} has no explicit qualification feature or supporting-only disposition"
    )]
    UnclassifiedOracleFixture { id: String },

    #[error(
        "supporting-only oracle fixture row {id:?} cannot find its canonical blocker owner {owner:?}"
    )]
    MissingSupportingOracleOwner { id: String, owner: String },

    #[error("oracle property fixture {id:?} has no source-owned property plan")]
    MissingOraclePropertyPlan { id: String },

    #[error("failed to parse blocker ledger {path}: {source}")]
    ParseBlockerLedger {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to parse qualification case ledger {path}: {source}")]
    ParseQualificationCases {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("qualification case ledger is invalid: {0}")]
    InvalidQualificationCases(String),

    #[error("blocker case {id:?} has unknown comparator {comparator:?}")]
    UnknownBlockerComparator { id: String, comparator: String },

    #[error("blocker case {id:?} has unknown status {status:?}")]
    UnknownBlockerStatus { id: String, status: String },

    #[error("blocker case {id:?} has invalid terminal Cargo selector: {reason}")]
    InvalidBlockerSelector { id: String, reason: String },

    #[error("blocker case {id:?} has no selected qualification feature")]
    UnclassifiedBlocker { id: String },

    #[error("blocker case {id:?} does not resolve to extracted upstream test {path}:{test}")]
    MissingBlockerUpstream {
        id: String,
        path: PathBuf,
        test: String,
    },

    #[error("blocker source-symbol case {id:?} matched {actual} anchors in {path}:{test}")]
    AmbiguousBlockerSource {
        id: String,
        path: PathBuf,
        test: String,
        actual: usize,
    },
}

#[derive(Debug, Deserialize)]
struct OracleEvidenceRow {
    id: String,
    upstream_source: PathBuf,
    comparator: String,
    argv: String,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockerLedgerEvidence {
    blockers: Vec<BlockerEvidenceGroup>,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockerEvidenceGroup {
    cases: Vec<BlockerEvidenceCase>,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockerEvidenceCase {
    id: String,
    upstream: BlockerUpstream,
    comparator: String,
    status: String,
    test: BlockerTest,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockerTest {
    selector: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockerUpstream {
    path: PathBuf,
    kind: String,
    test: String,
    subcase: String,
}

pub(super) fn generate(root: &RepoRoot) -> Result<QualificationManifest, InventoryError> {
    let stim = crate::validate_stim_source(root)
        .map_err(|source| InventoryError::StimSource(source.to_string().into_boxed_str()))?;
    let blocker_cases = read_blocker_cases(root)?;
    let direct_case_limit =
        MAX_CASES
            .checked_sub(blocker_cases.len())
            .ok_or(InventoryError::TooManyRecords {
                kind: "blocker cases",
                actual: blocker_cases.len(),
                limit: MAX_CASES,
            })?;
    let mut upstream_cases = generate_cpp_cases(root, direct_case_limit)?;
    let python_case_limit = direct_case_limit.saturating_sub(upstream_cases.len());
    upstream_cases.extend(generate_python_cases(root, python_case_limit)?);
    upstream_cases.extend(make_blocker_upstream_cases(
        root,
        &upstream_cases,
        &blocker_cases,
    )?);
    upstream_cases.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| left.subcase.cmp(&right.subcase))
    });
    ensure_limit("upstream cases", upstream_cases.len())?;

    let mut evidence_cases = upstream_cases
        .iter()
        .flat_map(make_upstream_evidence_cases)
        .collect::<Vec<_>>();

    let mut extracted_api = generate_rustdoc_inventory(&root.path, "stab-core", "stab_core")?;
    let cli_api = generate_rustdoc_inventory(&root.path, "stab-cli", "stab_cli")?;
    if extracted_api.format_version != cli_api.format_version {
        return Err(InventoryError::PublicApi(PublicApiError::InvalidField(
            "rustdoc format version mismatch",
        )));
    }
    extracted_api.items.extend(cli_api.items);
    extracted_api.items.sort();
    ensure_limit("public API items", extracted_api.items.len())?;
    let (mut public_api_items, api_evidence) = make_public_api_records(&extracted_api.items)?;
    evidence_cases.extend(api_evidence);
    let mut blocker_evidence = generate_blocker_evidence(&blocker_cases)?;
    evidence_cases.extend(generate_existing_oracle_evidence(
        root,
        &mut blocker_evidence,
    )?);
    evidence_cases.extend(blocker_evidence);
    qualification_cases::apply(
        root,
        &stim.tag,
        &stim.commit,
        &mut upstream_cases,
        &mut public_api_items,
        &mut evidence_cases,
    )?;
    evidence_cases.extend(super::resource::planned_evidence());
    evidence_cases.push(super::resource::existing_regression());
    evidence_cases.push(super::resource::existing_property_regression());
    evidence_cases.sort_by(|left, right| left.id.cmp(&right.id));
    super::execution_contract::assign_pr_tiers(&mut evidence_cases);
    ensure_limit("evidence cases", evidence_cases.len())?;

    let features = FeatureId::ALL
        .into_iter()
        .map(|id| FeatureRecord {
            id,
            performance_groups: id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
        })
        .collect();
    let mut manifest = QualificationManifest {
        schema_version: SCHEMA_VERSION,
        stim_version: stim.tag,
        stim_commit: stim.commit,
        rust_toolchain: RUST_TOOLCHAIN.to_string(),
        python_ast_version: PYTHON_AST_VERSION.to_string(),
        semantic_digest: SemanticDigest::ZERO,
        features,
        upstream_cases,
        public_api_items,
        evidence_cases,
    };
    manifest.semantic_digest = semantic_digest(&manifest)?;
    Ok(manifest)
}

pub(super) fn semantic_digest(
    manifest: &QualificationManifest,
) -> Result<SemanticDigest, InventoryError> {
    let mut payload = manifest.clone();
    payload.semantic_digest = SemanticDigest::ZERO;
    let bytes = serde_json::to_vec(&payload).map_err(InventoryError::Serialize)?;
    Ok(SemanticDigest::from_bytes(Sha256::digest(bytes).into()))
}

fn generate_cpp_cases(
    root: &RepoRoot,
    case_limit: usize,
) -> Result<Vec<UpstreamCase>, InventoryError> {
    let list_path = root.stim_source().join("file_lists").join("test_files");
    let list = read_utf8_bounded(&list_path, MAX_FILE_LIST_BYTES)?;
    let paths = parse_source_lines(&list, ".test.cc")?;
    if paths.len() != CPP_TEST_FILE_COUNT {
        return Err(InventoryError::WrongCppFileCount {
            actual: paths.len(),
            expected: CPP_TEST_FILE_COUNT,
        });
    }
    let mut cases = Vec::new();
    for path in paths {
        let source_path = root.stim_source().join(path.as_path());
        let source = read_utf8_bounded(&source_path, MAX_SOURCE_BYTES)?;
        let remaining = case_limit.saturating_sub(cases.len());
        for declaration in extract_cpp_test_cases_bounded(&source, remaining)? {
            cases.push(make_cpp_upstream_case(&path, declaration));
        }
    }
    Ok(cases)
}

fn generate_python_cases(
    root: &RepoRoot,
    case_limit: usize,
) -> Result<Vec<UpstreamCase>, InventoryError> {
    let output = crate::run_process(
        Path::new("git"),
        ["ls-files", "-z", "--", "*_test.py"],
        &[],
        Some(&root.stim_source()),
    )
    .map_err(|source| InventoryError::ListPythonTests(source.to_string().into_boxed_str()))?;
    if !output.success() {
        return Err(InventoryError::ListPythonTestsFailed {
            status: crate::process::display_status(output.status),
            stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
            stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
        });
    }
    let listed =
        std::str::from_utf8(&output.stdout.bytes).map_err(|_| InventoryError::NonUtf8PythonList)?;
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in listed.split('\0').filter(|value| !value.is_empty()) {
        let path = validate_relative_source_path(raw, "_test.py")?;
        if !seen.insert(path.clone()) {
            return Err(InventoryError::DuplicateSourcePath(raw.to_string()));
        }
        paths.push(path);
    }
    paths.sort();
    if paths.len() != PYTHON_TEST_FILE_COUNT {
        return Err(InventoryError::WrongPythonFileCount {
            actual: paths.len(),
            expected: PYTHON_TEST_FILE_COUNT,
        });
    }

    let mut contents = Vec::with_capacity(paths.len());
    let mut total_source_bytes = 0usize;
    for path in &paths {
        let absolute = root.stim_source().join(path.as_path());
        let content = read_utf8_bounded(&absolute, MAX_SOURCE_BYTES)?;
        total_source_bytes = total_source_bytes.checked_add(content.len()).ok_or(
            InventoryError::TooManyRecords {
                kind: "Python source bytes",
                actual: usize::MAX,
                limit: MAX_PYTHON_SOURCE_BYTES,
            },
        )?;
        if total_source_bytes > MAX_PYTHON_SOURCE_BYTES {
            return Err(InventoryError::TooManyRecords {
                kind: "Python source bytes",
                actual: total_source_bytes,
                limit: MAX_PYTHON_SOURCE_BYTES,
            });
        }
        contents.push(content);
    }
    let sources = paths
        .iter()
        .zip(&contents)
        .map(|(path, content)| {
            let path = path.as_path().to_str().ok_or_else(|| {
                InventoryError::InvalidSourcePath(format!("{:?}", path.as_path()))
            })?;
            Ok(PythonSource { path, content })
        })
        .collect::<Result<Vec<_>, InventoryError>>()?;
    let declarations = extract_python_test_cases_bounded(&sources, &root.path, case_limit)?;
    declarations
        .into_iter()
        .map(make_python_upstream_case)
        .collect()
}

fn make_cpp_upstream_case(
    path: &RelativeSourcePath,
    declaration: CppTestDeclaration,
) -> UpstreamCase {
    let classification = classify_upstream_case(path.as_path(), &declaration.symbol);
    let key = format!(
        "cpp\0{}\0{}\0{}",
        path,
        declaration.symbol,
        declaration.subcase.as_deref().unwrap_or("")
    );
    let id = stable_id(StableCaseDomain::UpstreamCpp, &key);
    let ownerships = if classification.disposition.is_executable_scope() {
        make_upstream_ownerships(&classification.feature_ids, &key)
    } else {
        Vec::new()
    };
    let parameterization = if declaration.subcase.is_some() {
        Parameterization::StaticSubcase
    } else {
        Parameterization::None
    };
    UpstreamCase {
        id,
        path: path.clone(),
        provenance: UpstreamProvenance::GtestCase,
        symbol: declaration.symbol,
        subcase: declaration.subcase.or_else(|| {
            (declaration.macro_name == "TEST_EACH_WORD_SIZE_W")
                .then(|| declaration.macro_name.to_string())
        }),
        parameterization,
        line: declaration.line,
        domain_ids: classification.feature_ids,
        disposition: classification.disposition,
        deferred_product: classification.deferred_product,
        reason: classification.reason.to_string(),
        ownerships,
    }
}

fn make_python_upstream_case(
    declaration: super::extract::PythonTestDeclaration,
) -> Result<UpstreamCase, InventoryError> {
    let path = validate_relative_source_path(&declaration.path, "_test.py")?;
    let classification = classify_upstream_case(path.as_path(), &declaration.symbol);
    let key = format!(
        "python\0{}\0{}\0{}",
        declaration.path,
        declaration.symbol,
        declaration.subcase.as_deref().unwrap_or("")
    );
    let id = stable_id(StableCaseDomain::UpstreamPython, &key);
    let ownerships = if classification.disposition.is_executable_scope() {
        make_upstream_ownerships(&classification.feature_ids, &key)
    } else {
        Vec::new()
    };
    let parameterization = if declaration.dynamic_parameters {
        Parameterization::DynamicFamily
    } else if declaration.subcase.is_some() {
        Parameterization::StaticSubcase
    } else {
        Parameterization::None
    };
    Ok(UpstreamCase {
        id,
        path,
        provenance: UpstreamProvenance::PytestCase,
        symbol: declaration.symbol,
        subcase: declaration.subcase,
        parameterization,
        line: declaration.line,
        domain_ids: classification.feature_ids,
        disposition: classification.disposition,
        deferred_product: classification.deferred_product,
        reason: classification.reason.to_string(),
        ownerships,
    })
}

fn make_upstream_ownerships(feature_ids: &[FeatureId], key: &str) -> Vec<UpstreamOwnership> {
    feature_ids
        .iter()
        .map(|feature_id| UpstreamOwnership {
            feature_id: *feature_id,
            comparator: default_comparator(*feature_id),
            owner_case_id: stable_id(
                StableCaseDomain::EvidenceUpstream,
                &format!("{key}\0{}", feature_id.as_str()),
            ),
        })
        .collect()
}

fn make_upstream_evidence_cases(case: &UpstreamCase) -> Vec<EvidenceCase> {
    if case.disposition != super::model::UpstreamDisposition::SemanticMining {
        return Vec::new();
    }
    case.ownerships
        .iter()
        .map(|ownership| {
            make_planned_evidence_case(
                ownership.owner_case_id.clone(),
                ownership.feature_id,
                EvidenceProvenance::UpstreamSemanticCase,
                case.id.to_string(),
                ownership.comparator,
                planned_selector(ownership.feature_id, case.id.as_str()),
            )
        })
        .collect()
}

fn make_public_api_records(
    extracted: &[ExtractedPublicApiItem],
) -> Result<(Vec<PublicApiItem>, Vec<EvidenceCase>), InventoryError> {
    let mut public_items = Vec::with_capacity(extracted.len());
    let mut evidence_by_id = BTreeMap::<CaseId, EvidenceCase>::new();
    let mut ids = BTreeSet::new();
    let mut owner_features = BTreeMap::new();
    for item in extracted.iter().filter(|item| item.path == item.owner_path) {
        let feature_id =
            classify_public_api_source(&item.crate_name, &item.source_path, &item.owner_path)
                .ok_or_else(|| InventoryError::UnclassifiedPublicApi {
                    crate_name: item.crate_name.clone(),
                    path: item.owner_path.clone(),
                    source_path: item.source_path.clone(),
                })?;
        owner_features.insert(
            (item.crate_name.as_str(), item.owner_path.as_str()),
            feature_id,
        );
    }
    for item in extracted {
        let owner_feature_id = owner_features
            .get(&(item.crate_name.as_str(), item.owner_path.as_str()))
            .copied()
            .ok_or_else(|| InventoryError::UnclassifiedPublicApi {
                crate_name: item.crate_name.clone(),
                path: item.owner_path.clone(),
                source_path: item.source_path.clone(),
            })?;
        let feature_id =
            classify_public_api_source(&item.crate_name, &item.source_path, &item.path)
                .ok_or_else(|| InventoryError::UnclassifiedPublicApi {
                    crate_name: item.crate_name.clone(),
                    path: item.path.clone(),
                    source_path: item.source_path.clone(),
                })?;
        let item_key = format!("{}\0{}\0{:?}", item.crate_name, item.path, item.kind);
        let item_id = stable_id(StableCaseDomain::ApiItem, &item_key);
        if !ids.insert(item_id.clone()) {
            return Err(InventoryError::StableIdCollision(item_id.to_string()));
        }
        let evidence_owner_path = if feature_id == owner_feature_id {
            &item.owner_path
        } else {
            &item.path
        };
        let owner_key = format!("{}\0{}", item.crate_name, evidence_owner_path);
        let owner_case_id = stable_id(StableCaseDomain::EvidenceApi, &owner_key);
        evidence_by_id
            .entry(owner_case_id.clone())
            .or_insert_with(|| {
                make_planned_evidence_case(
                    owner_case_id.clone(),
                    feature_id,
                    EvidenceProvenance::PublicRustApi,
                    evidence_owner_path.clone(),
                    default_comparator(feature_id),
                    planned_api_selector(&item.crate_name, &owner_case_id),
                )
            });
        public_items.push(PublicApiItem {
            id: item_id,
            feature_id,
            crate_name: item.crate_name.clone(),
            path: ApiPath::try_new(item.path.clone()).map_err(|_| {
                InventoryError::UnclassifiedPublicApi {
                    crate_name: item.crate_name.clone(),
                    path: item.path.clone(),
                    source_path: item.source_path.clone(),
                }
            })?,
            kind: item.kind,
            source_path: RelativeSourcePath::try_new(item.source_path.clone()).map_err(|_| {
                InventoryError::InvalidSourcePath(format!("{:?}", item.source_path))
            })?,
            source_line: item.source_line,
            owner_case_id,
            performance_groups: feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
        });
    }
    public_items.sort_by(|left, right| {
        left.crate_name
            .cmp(&right.crate_name)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    Ok((public_items, evidence_by_id.into_values().collect()))
}

fn generate_existing_oracle_evidence(
    root: &RepoRoot,
    blocker_evidence: &mut [EvidenceCase],
) -> Result<Vec<EvidenceCase>, InventoryError> {
    let path = root.fixture_manifest();
    let bytes = crate::safe_file::read_regular_file_bounded(&path, MAX_ORACLE_MANIFEST_BYTES)
        .map_err(|source| InventoryError::Read {
            path: path.clone(),
            reason: source.to_string().into_boxed_str(),
        })?;
    let mut reader = csv::ReaderBuilder::new().from_reader(bytes.as_slice());
    let mut evidence = Vec::new();
    for row in reader.deserialize::<OracleEvidenceRow>() {
        let row = row.map_err(|source| InventoryError::ParseOracleManifest {
            path: path.clone(),
            source,
        })?;
        if row.status != "implemented" {
            continue;
        }
        let source_text = row.upstream_source.to_str().ok_or_else(|| {
            InventoryError::InvalidSourcePath(format!("{:?}", row.upstream_source))
        })?;
        drop(validate_relative_source_path(source_text, "")?);
        let comparator = match row.comparator.as_str() {
            "exact-output" => super::model::Comparator::ExactBytes,
            "help-health" | "structural" => super::model::Comparator::Structural,
            "property" => super::model::Comparator::Property,
            "statistical" => super::model::Comparator::Statistical,
            _ => {
                return Err(InventoryError::UnknownOracleComparator {
                    id: row.id,
                    comparator: row.comparator,
                });
            }
        };
        let exact_cargo_selector = if let Some(parts) =
            CargoTestSelector::normalize_fixture_argv(&row.argv).map_err(|reason| {
                InventoryError::InvalidOracleSelector {
                    id: row.id.clone(),
                    reason: reason.to_string(),
                }
            })? {
            let parsed = CargoTestSelector::parse(&parts).map_err(|reason| {
                InventoryError::InvalidOracleSelector {
                    id: row.id.clone(),
                    reason: reason.to_string(),
                }
            })?;
            if !parsed.is_exact() {
                return Err(InventoryError::InvalidOracleSelector {
                    id: row.id.clone(),
                    reason: "selector is not exact".to_string(),
                });
            }
            Some(parts)
        } else {
            None
        };
        if let Some(selector) = exact_cargo_selector.as_ref()
            && let Some(owner) = blocker_evidence.iter_mut().find(|case| {
                case.primary_selector.kind == SelectorKind::CargoTest
                    && case.primary_selector.value == *selector
            })
        {
            owner.supporting_selectors.push(EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::OracleFixture,
                value: vec![row.id],
            });
            continue;
        }
        if let Some((feature_id, owner_source_id, _reason)) =
            supporting_only_oracle_fixture(&row.id)
        {
            let owner = blocker_evidence
                .iter_mut()
                .find(|case| case.feature_id == feature_id && case.source_id == owner_source_id)
                .ok_or_else(|| InventoryError::MissingSupportingOracleOwner {
                    id: row.id.clone(),
                    owner: owner_source_id.to_string(),
                })?;
            owner.supporting_selectors.push(EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::OracleFixture,
                value: vec![row.id.clone()],
            });
            if let Some(value) = broad_cargo_selector(&row.argv) {
                owner.supporting_selectors.push(EvidenceSelector {
                    state: EvidenceState::Existing,
                    kind: SelectorKind::CargoTest,
                    value,
                });
            }
            continue;
        }
        let classification = classify_upstream_path(&row.upstream_source);
        let feature_id = oracle_feature_override(&row.id)
            .or_else(|| infer_feature_from_oracle_argv(&row.argv))
            .or_else(|| classification.feature_ids.first().copied())
            .ok_or_else(|| InventoryError::UnclassifiedOracleFixture { id: row.id.clone() })?;
        let broad_cargo_selector = broad_cargo_selector(&row.argv);
        let status = if broad_cargo_selector.is_some() {
            EvidenceStatus::Planned
        } else {
            EvidenceStatus::Implemented
        };
        let id = stable_id(StableCaseDomain::EvidenceOracle, &row.id);
        let statistical_plan = statistical_plan_reference(
            comparator,
            status,
            EvidenceProvenance::OracleFixture,
            &row.id,
            &id,
        );
        let property_plan =
            property_plan::oracle_reference(root, comparator, status, &row.id, &id)?;
        evidence.push(EvidenceCase {
            id,
            feature_id,
            behavioral_surface: oracle_behavioral_surface(&row.argv),
            provenance: EvidenceProvenance::OracleFixture,
            source_id: row.id.clone(),
            comparator,
            execution: super::execution_contract::for_status(status),
            statistical_plan,
            property_plan,
            primary_selector: exact_cargo_selector
                .clone()
                .map(|value| EvidenceSelector {
                    state: EvidenceState::Existing,
                    kind: SelectorKind::CargoTest,
                    value,
                })
                .unwrap_or_else(|| EvidenceSelector {
                    state: if status == EvidenceStatus::Planned {
                        EvidenceState::Planned
                    } else {
                        EvidenceState::Existing
                    },
                    kind: SelectorKind::OracleFixture,
                    value: vec![row.id.clone()],
                }),
            supporting_selectors: {
                let mut selectors = broad_cargo_selector
                    .into_iter()
                    .map(|value| EvidenceSelector {
                        state: EvidenceState::Existing,
                        kind: SelectorKind::CargoTest,
                        value,
                    })
                    .collect::<Vec<_>>();
                if exact_cargo_selector.is_some() {
                    selectors.push(EvidenceSelector {
                        state: EvidenceState::Existing,
                        kind: SelectorKind::OracleFixture,
                        value: vec![row.id.clone()],
                    });
                }
                selectors
            },
            resource_contract: ResourceContract {
                kind: ResourceKind::NotApplicable,
                detail: "This imported oracle fixture proves its semantic comparator only; separate CQ cases own hostile-input and resource boundaries."
                    .to_string(),
            },
            negative_axes: Vec::new(),
            performance_groups: feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
            deferred_product: None,
            status,
        });
        if evidence.len() > MAX_ORACLE_ROWS {
            return Err(InventoryError::TooManyOracleRows {
                limit: MAX_ORACLE_ROWS,
            });
        }
    }
    Ok(evidence)
}

fn oracle_feature_override(id: &str) -> Option<FeatureId> {
    match id {
        "coverage-util-bot-twiddle" => Some(FeatureId::BitKernels),
        _ => None,
    }
}

fn supporting_only_oracle_fixture(id: &str) -> Option<(FeatureId, &'static str, &'static str)> {
    match id {
        "pf5-detecting-regions-clifford-rust" => Some((
            FeatureId::FlowUtils,
            "pfm5-detecting-regions-simple",
            "The broad Cargo filter is supporting provenance for the source-owned detecting-regions blocker and cannot own an atomic qualification case.",
        )),
        _ => None,
    }
}

fn broad_cargo_selector(argv: &str) -> Option<Vec<String>> {
    let tokens = argv.split('|').collect::<Vec<_>>();
    if tokens.first().copied() != Some("cargo-test") || tokens.contains(&"--exact") {
        return None;
    }
    let mut selector = vec!["cargo".to_string(), "test".to_string()];
    selector.extend(tokens.into_iter().skip(1).map(ToOwned::to_owned));
    selector.push("--quiet".to_string());
    Some(selector)
}

fn read_blocker_cases(root: &RepoRoot) -> Result<Vec<BlockerEvidenceCase>, InventoryError> {
    let path = root.blocker_ledger();
    let bytes = crate::safe_file::read_regular_file_bounded(&path, MAX_BLOCKER_LEDGER_BYTES)
        .map_err(|source| InventoryError::Read {
            path: path.clone(),
            reason: source.to_string().into_boxed_str(),
        })?;
    let ledger: BlockerLedgerEvidence = serde_json::from_slice(&bytes)
        .map_err(|source| InventoryError::ParseBlockerLedger { path, source })?;
    Ok(ledger
        .blockers
        .into_iter()
        .flat_map(|blocker| blocker.cases)
        .collect())
}

fn make_blocker_upstream_cases(
    root: &RepoRoot,
    extracted: &[UpstreamCase],
    blocker_cases: &[BlockerEvidenceCase],
) -> Result<Vec<UpstreamCase>, InventoryError> {
    let mut cases = Vec::with_capacity(blocker_cases.len());
    for blocker in blocker_cases {
        let (provenance, line) = match blocker.upstream.kind.as_str() {
            "gtest-case" | "pytest-case" => {
                let anchor = extracted
                    .iter()
                    .find(|case| {
                        case.path.as_path() == blocker.upstream.path
                            && (case.symbol == blocker.upstream.test
                                || (case
                                    .subcase
                                    .as_deref()
                                    .is_some_and(|subcase| subcase.starts_with("W="))
                                    && word_size_base_symbol(&case.symbol)
                                        == blocker.upstream.test))
                    })
                    .ok_or_else(|| InventoryError::MissingBlockerUpstream {
                        id: blocker.id.clone(),
                        path: blocker.upstream.path.clone(),
                        test: blocker.upstream.test.clone(),
                    })?;
                (anchor.provenance, anchor.line)
            }
            "source-symbol" => (
                UpstreamProvenance::SourceSymbol,
                resolve_source_symbol_line(root, blocker)?,
            ),
            _ => {
                return Err(InventoryError::MissingBlockerUpstream {
                    id: blocker.id.clone(),
                    path: blocker.upstream.path.clone(),
                    test: blocker.upstream.test.clone(),
                });
            }
        };
        let feature_id = blocker_feature(blocker)?;
        let comparator = blocker_comparator(blocker)?;
        cases.push(UpstreamCase {
            id: stable_id(StableCaseDomain::UpstreamBlocker, &blocker.id),
            path: validate_relative_source_path(
                blocker
                    .upstream
                    .path
                    .to_str()
                    .ok_or_else(|| {
                        InventoryError::InvalidSourcePath(format!(
                            "{:?}",
                            blocker.upstream.path
                        ))
                    })?,
                "",
            )?,
            provenance,
            symbol: blocker.upstream.test.clone(),
            subcase: Some(blocker.upstream.subcase.clone()),
            parameterization: Parameterization::StaticSubcase,
            line,
            domain_ids: vec![feature_id],
            disposition: super::model::UpstreamDisposition::PortedRust,
            deferred_product: None,
            reason: "The source-owned blocker ledger binds this exact upstream subcase to implemented or evidence-close Rust evidence."
                .to_string(),
            ownerships: vec![UpstreamOwnership {
                feature_id,
                comparator,
                owner_case_id: stable_id(StableCaseDomain::EvidenceBlocker, &blocker.id),
            }],
        });
    }
    Ok(cases)
}

fn word_size_base_symbol(symbol: &str) -> &str {
    ["_64", "_128", "_256"]
        .into_iter()
        .find_map(|suffix| symbol.strip_suffix(suffix))
        .unwrap_or(symbol)
}

fn resolve_source_symbol_line(
    root: &RepoRoot,
    blocker: &BlockerEvidenceCase,
) -> Result<u32, InventoryError> {
    let source_text =
        blocker.upstream.path.to_str().ok_or_else(|| {
            InventoryError::InvalidSourcePath(format!("{:?}", blocker.upstream.path))
        })?;
    drop(validate_relative_source_path(source_text, "")?);
    let absolute = root.stim_source().join(&blocker.upstream.path);
    let source = read_utf8_bounded(&absolute, MAX_SOURCE_BYTES)?;
    let matches = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(&blocker.upstream.test))
        .map(|(index, _)| index.saturating_add(1))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(InventoryError::AmbiguousBlockerSource {
            id: blocker.id.clone(),
            path: blocker.upstream.path.clone(),
            test: blocker.upstream.test.clone(),
            actual: matches.len(),
        });
    }
    Ok(u32::try_from(matches.first().copied().unwrap_or(usize::MAX)).unwrap_or(u32::MAX))
}

fn generate_blocker_evidence(
    blocker_cases: &[BlockerEvidenceCase],
) -> Result<Vec<EvidenceCase>, InventoryError> {
    let mut evidence = Vec::new();
    for case in blocker_cases {
        let source_text = case.upstream.path.to_str().ok_or_else(|| {
            InventoryError::InvalidSourcePath(format!("{:?}", case.upstream.path))
        })?;
        drop(validate_relative_source_path(source_text, "")?);
        let feature_id = blocker_feature(case)?;
        let comparator = blocker_comparator(case)?;
        let status = match case.status.as_str() {
            "implemented" => EvidenceStatus::Implemented,
            "evidence-close" => EvidenceStatus::EvidenceClose,
            _ => {
                return Err(InventoryError::UnknownBlockerStatus {
                    id: case.id.clone(),
                    status: case.status.clone(),
                });
            }
        };
        let evidence_id = stable_id(StableCaseDomain::EvidenceBlocker, &case.id);
        let statistical_plan = statistical_plan_reference(
            comparator,
            status,
            EvidenceProvenance::BlockerLedger,
            &case.id,
            &evidence_id,
        );
        let property_plan = None;
        let terminal_selector =
            CargoTestSelector::parse(&case.test.selector).map_err(|reason| {
                InventoryError::InvalidBlockerSelector {
                    id: case.id.clone(),
                    reason: reason.to_string(),
                }
            })?;
        if !terminal_selector.is_exact() {
            return Err(InventoryError::InvalidBlockerSelector {
                id: case.id.clone(),
                reason: "selector is not exact".to_string(),
            });
        }
        evidence.push(EvidenceCase {
            id: evidence_id,
            feature_id,
            behavioral_surface: behavioral_surface_for_feature(
                feature_id,
                EvidenceProvenance::BlockerLedger,
            ),
            provenance: EvidenceProvenance::BlockerLedger,
            source_id: case.id.clone(),
            comparator,
            execution: super::execution_contract::for_status(status),
            statistical_plan,
            property_plan,
            primary_selector: EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::CargoTest,
                value: case.test.selector.clone(),
            },
            supporting_selectors: vec![EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::OpsCheck,
                value: vec!["blocker-ledger".to_string(), case.id.clone()],
            }],
            resource_contract: semantic_only_resource_contract(),
            negative_axes: Vec::new(),
            performance_groups: feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
            deferred_product: None,
            status,
        });
    }
    Ok(evidence)
}

fn blocker_feature(case: &BlockerEvidenceCase) -> Result<FeatureId, InventoryError> {
    if case.id.starts_with("pfm3-contract-") {
        return Ok(FeatureId::GateContract);
    }
    if case.id.starts_with("pfm3-analyzer-")
        || case.id.starts_with("pfm6-analyzer-")
        || case.id.starts_with("pfm6-matched-error-")
    {
        return Ok(FeatureId::Analyzer);
    }
    let classification = classify_upstream_path(&case.upstream.path);
    let feature_ids = classification.feature_ids;
    if let [feature_id] = feature_ids.as_slice() {
        Ok(*feature_id)
    } else {
        Err(InventoryError::UnclassifiedBlocker {
            id: case.id.clone(),
        })
    }
}

fn blocker_comparator(
    case: &BlockerEvidenceCase,
) -> Result<super::model::Comparator, InventoryError> {
    match case.comparator.as_str() {
        "exact" => Ok(super::model::Comparator::ExactValue),
        "structural" => Ok(super::model::Comparator::Structural),
        "statistical" => Ok(super::model::Comparator::Statistical),
        "error-class" => Ok(super::model::Comparator::ErrorClass),
        "semantic-invariant" => Ok(super::model::Comparator::SemanticInvariant),
        "state-equivalence" => Ok(super::model::Comparator::StateEquivalence),
        _ => Err(InventoryError::UnknownBlockerComparator {
            id: case.id.clone(),
            comparator: case.comparator.clone(),
        }),
    }
}

fn parse_source_lines(
    content: &str,
    suffix: &str,
) -> Result<Vec<RelativeSourcePath>, InventoryError> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for line in content.lines() {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }
        let path = validate_relative_source_path(raw, suffix)?;
        if !seen.insert(path.clone()) {
            return Err(InventoryError::DuplicateSourcePath(raw.to_string()));
        }
        paths.push(path);
    }
    Ok(paths)
}

fn validate_relative_source_path(
    value: &str,
    suffix: &str,
) -> Result<RelativeSourcePath, InventoryError> {
    if value.is_empty()
        || value.len() > MAX_SOURCE_PATH_BYTES
        || value.contains('\\')
        || !value.ends_with(suffix)
        || value.chars().any(char::is_control)
    {
        return Err(InventoryError::InvalidSourcePath(value.to_string()));
    }
    RelativeSourcePath::try_new(PathBuf::from(value))
        .map_err(|_| InventoryError::InvalidSourcePath(value.to_string()))
}

fn read_utf8_bounded(path: &Path, limit: usize) -> Result<String, InventoryError> {
    let bytes = crate::safe_file::read_regular_file_bounded(path, limit).map_err(|source| {
        InventoryError::Read {
            path: path.to_path_buf(),
            reason: source.to_string().into_boxed_str(),
        }
    })?;
    String::from_utf8(bytes).map_err(|_| InventoryError::NonUtf8 {
        path: path.to_path_buf(),
    })
}

fn ensure_limit(kind: &'static str, actual: usize) -> Result<(), InventoryError> {
    if actual > MAX_CASES {
        Err(InventoryError::TooManyRecords {
            kind,
            actual,
            limit: MAX_CASES,
        })
    } else {
        Ok(())
    }
}

pub(super) fn stable_id(domain: StableCaseDomain, key: &str) -> CaseId {
    let digest = Sha256::digest(key.as_bytes());
    let suffix = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    CaseId::from_stable_suffix(domain, &suffix)
}

#[cfg(test)]
#[path = "inventory/tests.rs"]
mod tests;
