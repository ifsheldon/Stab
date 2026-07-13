use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::root::RepoRoot;

const HOST_POLICY_PATH: &str = "benchmarks/qualification-host-policy.json";
const HOST_POLICY_SCHEMA_VERSION: u32 = 2;
const MAX_HOST_POLICY_BYTES: usize = 1 << 20;
const MAX_THERMAL_ZONES: usize = 128;
const MEM_AVAILABLE_FIELD: &str = "MemAvailable:";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostPolicy {
    schema_version: u32,
    profiles: Vec<HostProfile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostProfile {
    id: String,
    operating_system: String,
    architecture: String,
    cpu_selection: CpuSelection,
    max_load_per_allowed_cpu: String,
    minimum_available_memory_bytes: u64,
    require_no_swap_activity: bool,
    require_frequency_governor: bool,
    allowed_frequency_governors: Vec<String>,
    require_thermal_probe: bool,
    maximum_temperature_millidegrees_celsius: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum CpuSelection {
    LowestAllowed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostEvidence {
    pub(super) policy_sha256: String,
    pub(super) profile_id: String,
    pub(super) operating_system: String,
    pub(super) architecture: String,
    pub(super) allowed_cpus: Vec<usize>,
    pub(super) logical_cpu_count: usize,
    pub(super) selected_cpu: usize,
    pub(super) cpu_identity: String,
    pub(super) load_one_before: f64,
    pub(super) load_one_after: f64,
    pub(super) maximum_load_one: f64,
    pub(super) available_memory_before_bytes: u64,
    pub(super) available_memory_after_bytes: u64,
    pub(super) minimum_available_memory_bytes: u64,
    pub(super) swap_in_before: u64,
    pub(super) swap_in_after: u64,
    pub(super) swap_out_before: u64,
    pub(super) swap_out_after: u64,
    pub(super) frequency_governor_before: Option<String>,
    pub(super) frequency_governor_after: Option<String>,
    pub(super) frequency_khz_before: Option<u64>,
    pub(super) frequency_khz_after: Option<u64>,
    pub(super) maximum_temperature_millidegrees_celsius: i64,
    pub(super) thermal_readings_before: Vec<ThermalReading>,
    pub(super) thermal_readings_after: Vec<ThermalReading>,
    pub(super) thermal_probe_available: bool,
    pub(super) verified: bool,
    pub(super) violations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ThermalReading {
    pub(super) zone: String,
    pub(super) kind: String,
    pub(super) millidegrees_celsius: i64,
}

#[derive(Debug)]
pub(crate) struct HostGuard {
    profile: HostProfile,
    allow_unverified: bool,
    evidence: HostEvidence,
}

impl HostGuard {
    pub(crate) fn prepare(root: &RepoRoot, allow_unverified: bool) -> Result<Self, HostError> {
        ensure_linux()?;
        let (policy, bytes) = load_policy(root)?;
        validate_policy(&policy)?;
        let profile = select_profile(policy)?;
        let allowed_cpus = allowed_cpus()?;
        let selected_cpu = match profile.cpu_selection {
            CpuSelection::LowestAllowed => allowed_cpus
                .first()
                .copied()
                .ok_or(HostError::EmptyAffinity)?,
        };
        let maximum_load_one = parse_positive_finite(
            "max_load_per_allowed_cpu",
            &profile.max_load_per_allowed_cpu,
        )? * allowed_cpus.len() as f64;
        let before = HostSnapshot::read(selected_cpu)?;
        let policy_sha256 = sha256_hex(&bytes);
        let mut evidence = HostEvidence {
            policy_sha256,
            profile_id: profile.id.clone(),
            operating_system: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            logical_cpu_count: allowed_cpus.len(),
            allowed_cpus,
            selected_cpu,
            cpu_identity: cpu_identity()?,
            load_one_before: before.load_one,
            load_one_after: before.load_one,
            maximum_load_one,
            available_memory_before_bytes: before.available_memory_bytes,
            available_memory_after_bytes: before.available_memory_bytes,
            minimum_available_memory_bytes: profile.minimum_available_memory_bytes,
            swap_in_before: before.swap_in,
            swap_in_after: before.swap_in,
            swap_out_before: before.swap_out,
            swap_out_after: before.swap_out,
            frequency_governor_before: before.frequency_governor.clone(),
            frequency_governor_after: before.frequency_governor.clone(),
            frequency_khz_before: before.frequency_khz,
            frequency_khz_after: before.frequency_khz,
            maximum_temperature_millidegrees_celsius: profile
                .maximum_temperature_millidegrees_celsius,
            thermal_readings_before: before.thermal_readings.clone(),
            thermal_readings_after: before.thermal_readings.clone(),
            thermal_probe_available: !before.thermal_readings.is_empty(),
            verified: true,
            violations: Vec::new(),
        };
        append_snapshot_violations(&profile, &mut evidence, &before, true);
        enforce_or_mark_unverified(&mut evidence, allow_unverified)?;
        Ok(Self {
            profile,
            allow_unverified,
            evidence,
        })
    }

    pub(crate) fn selected_cpu(&self) -> usize {
        self.evidence.selected_cpu
    }

    pub(crate) fn finish(mut self) -> Result<HostEvidence, HostError> {
        let after = HostSnapshot::read(self.evidence.selected_cpu)?;
        self.evidence.load_one_after = after.load_one;
        self.evidence.available_memory_after_bytes = after.available_memory_bytes;
        self.evidence.swap_in_after = after.swap_in;
        self.evidence.swap_out_after = after.swap_out;
        self.evidence.frequency_governor_after = after.frequency_governor.clone();
        self.evidence.frequency_khz_after = after.frequency_khz;
        self.evidence.thermal_readings_after = after.thermal_readings.clone();
        self.evidence.thermal_probe_available = !self.evidence.thermal_readings_before.is_empty()
            && !self.evidence.thermal_readings_after.is_empty();
        append_snapshot_violations(&self.profile, &mut self.evidence, &after, false);
        append_swap_violation(&self.profile, &mut self.evidence, &after);
        if self.evidence.frequency_governor_before != self.evidence.frequency_governor_after {
            self.evidence.violations.push(format!(
                "frequency governor changed during the run: {:?} -> {:?}",
                self.evidence.frequency_governor_before, self.evidence.frequency_governor_after
            ));
        }
        enforce_or_mark_unverified(&mut self.evidence, self.allow_unverified)?;
        Ok(self.evidence)
    }
}

pub(super) fn check_policy(root: &RepoRoot) -> Result<(), HostError> {
    let (policy, _) = load_policy(root)?;
    validate_policy(&policy)
}

fn load_policy(root: &RepoRoot) -> Result<(HostPolicy, Vec<u8>), HostError> {
    let path = root.path.join(HOST_POLICY_PATH);
    let bytes =
        crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_HOST_POLICY_BYTES)
            .map_err(|error| HostError::PolicyRead(error.to_string()))?;
    let policy = serde_json::from_slice(&bytes).map_err(HostError::PolicyJson)?;
    Ok((policy, bytes))
}

fn validate_policy(policy: &HostPolicy) -> Result<(), HostError> {
    if policy.schema_version != HOST_POLICY_SCHEMA_VERSION {
        return Err(HostError::SchemaVersion {
            actual: policy.schema_version,
            expected: HOST_POLICY_SCHEMA_VERSION,
        });
    }
    if policy.profiles.is_empty() {
        return Err(HostError::MissingProfiles);
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut platforms = std::collections::BTreeSet::new();
    for profile in &policy.profiles {
        if profile.id.is_empty()
            || profile.id.len() > 128
            || !profile.id.is_ascii()
            || !ids.insert(&profile.id)
        {
            return Err(HostError::InvalidProfileId(profile.id.clone()));
        }
        if profile.operating_system != "linux"
            || !matches!(profile.architecture.as_str(), "x86_64" | "aarch64")
            || !platforms.insert((&profile.operating_system, &profile.architecture))
        {
            return Err(HostError::InvalidProfilePlatform {
                operating_system: profile.operating_system.clone(),
                architecture: profile.architecture.clone(),
            });
        }
        parse_positive_finite(
            "max_load_per_allowed_cpu",
            &profile.max_load_per_allowed_cpu,
        )?;
        if profile.minimum_available_memory_bytes == 0 || !profile.require_no_swap_activity {
            return Err(HostError::IncompleteProfile(profile.id.clone()));
        }
        if profile.require_frequency_governor
            && (profile.allowed_frequency_governors.is_empty()
                || profile
                    .allowed_frequency_governors
                    .iter()
                    .any(|governor| governor.is_empty() || !governor.is_ascii()))
        {
            return Err(HostError::IncompleteProfile(profile.id.clone()));
        }
        if profile.require_thermal_probe && profile.maximum_temperature_millidegrees_celsius <= 0 {
            return Err(HostError::IncompleteProfile(profile.id.clone()));
        }
    }
    Ok(())
}

fn select_profile(policy: HostPolicy) -> Result<HostProfile, HostError> {
    let mut matching = policy.profiles.into_iter().filter(|profile| {
        profile.operating_system == std::env::consts::OS
            && profile.architecture == std::env::consts::ARCH
    });
    let profile = matching.next().ok_or_else(|| HostError::MissingProfile {
        operating_system: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
    })?;
    if matching.next().is_some() {
        return Err(HostError::DuplicateProfile {
            operating_system: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
        });
    }
    Ok(profile)
}

fn allowed_cpus() -> Result<Vec<usize>, HostError> {
    let set = rustix::thread::sched_getaffinity(None).map_err(HostError::Affinity)?;
    let cpus = (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| set.is_set(*cpu))
        .collect::<Vec<_>>();
    if cpus.is_empty() {
        Err(HostError::EmptyAffinity)
    } else {
        Ok(cpus)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct HostSnapshot {
    load_one: f64,
    available_memory_bytes: u64,
    swap_in: u64,
    swap_out: u64,
    frequency_governor: Option<String>,
    frequency_khz: Option<u64>,
    thermal_readings: Vec<ThermalReading>,
}

impl HostSnapshot {
    fn read(cpu: usize) -> Result<Self, HostError> {
        let loadavg = std::fs::read_to_string("/proc/loadavg").map_err(HostError::LoadAverage)?;
        let load_one = loadavg
            .split_ascii_whitespace()
            .next()
            .ok_or(HostError::MalformedLoadAverage)
            .and_then(|value| parse_nonnegative_finite("load average", value))?;
        let meminfo = std::fs::read_to_string("/proc/meminfo").map_err(HostError::MemoryInfo)?;
        let available_memory_bytes = parse_kib_field(&meminfo, MEM_AVAILABLE_FIELD)?;
        let vmstat = std::fs::read_to_string("/proc/vmstat").map_err(HostError::VmStat)?;
        Ok(Self {
            load_one,
            available_memory_bytes,
            swap_in: parse_counter_field(&vmstat, "pswpin")?,
            swap_out: parse_counter_field(&vmstat, "pswpout")?,
            frequency_governor: read_optional_probe(&format!(
                "/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_governor"
            ))?,
            frequency_khz: read_optional_probe(&format!(
                "/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq"
            ))?
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| HostError::MalformedOptionalProbe("scaling_cur_freq"))
            })
            .transpose()?,
            thermal_readings: read_thermal_zones()?,
        })
    }
}

fn append_snapshot_violations(
    profile: &HostProfile,
    evidence: &mut HostEvidence,
    snapshot: &HostSnapshot,
    before: bool,
) {
    let phase = if before { "before" } else { "after" };
    if snapshot.load_one > evidence.maximum_load_one {
        evidence.violations.push(format!(
            "one-minute load average {phase} the run is {:.3}, exceeding {:.3}",
            snapshot.load_one, evidence.maximum_load_one
        ));
    }
    if snapshot.available_memory_bytes < profile.minimum_available_memory_bytes {
        evidence.violations.push(format!(
            "available memory {phase} the run is {}, below {} bytes",
            snapshot.available_memory_bytes, profile.minimum_available_memory_bytes
        ));
    }
    match snapshot.frequency_governor.as_deref() {
        Some(governor)
            if !profile
                .allowed_frequency_governors
                .iter()
                .any(|allowed| allowed == governor) =>
        {
            evidence.violations.push(format!(
                "frequency governor {phase} the run is {governor:?}, not one of {:?}",
                profile.allowed_frequency_governors
            ));
        }
        None if profile.require_frequency_governor => evidence.violations.push(format!(
            "frequency governor probe is unavailable {phase} the run"
        )),
        _ => {}
    }
    if snapshot.thermal_readings.is_empty() {
        if profile.require_thermal_probe {
            evidence
                .violations
                .push(format!("thermal probe is unavailable {phase} the run"));
        }
    } else {
        for reading in &snapshot.thermal_readings {
            if reading.millidegrees_celsius > profile.maximum_temperature_millidegrees_celsius {
                evidence.violations.push(format!(
                    "thermal zone {} ({}) is {} millidegrees Celsius {phase} the run, exceeding {}",
                    reading.zone,
                    reading.kind,
                    reading.millidegrees_celsius,
                    profile.maximum_temperature_millidegrees_celsius
                ));
            }
        }
    }
    if !before
        && snapshot
            .thermal_readings
            .iter()
            .map(|reading| (&reading.zone, &reading.kind))
            .collect::<std::collections::BTreeSet<_>>()
            != evidence
                .thermal_readings_before
                .iter()
                .map(|reading| (&reading.zone, &reading.kind))
                .collect::<std::collections::BTreeSet<_>>()
    {
        evidence
            .violations
            .push("thermal probe zone set changed during the run".to_string());
    }
}

fn append_swap_violation(profile: &HostProfile, evidence: &mut HostEvidence, after: &HostSnapshot) {
    if profile.require_no_swap_activity
        && (after.swap_in != evidence.swap_in_before || after.swap_out != evidence.swap_out_before)
    {
        evidence.violations.push(format!(
            "swap activity changed during the run: pswpin {} -> {}, pswpout {} -> {}",
            evidence.swap_in_before, after.swap_in, evidence.swap_out_before, after.swap_out
        ));
    }
}

fn enforce_or_mark_unverified(
    evidence: &mut HostEvidence,
    allow_unverified: bool,
) -> Result<(), HostError> {
    evidence.violations.sort();
    evidence.violations.dedup();
    evidence.verified = evidence.violations.is_empty();
    if evidence.verified || allow_unverified {
        Ok(())
    } else {
        Err(HostError::PolicyViolation(evidence.violations.join("; ")))
    }
}

fn parse_kib_field(text: &str, field: &'static str) -> Result<u64, HostError> {
    let line = text
        .lines()
        .find(|line| line.starts_with(field))
        .ok_or(HostError::MissingProbeField(field))?;
    let mut fields = line.split_ascii_whitespace();
    if fields.next() != Some(field) {
        return Err(HostError::MalformedProbeField(field));
    }
    let kib = fields
        .next()
        .ok_or(HostError::MalformedProbeField(field))?
        .parse::<u64>()
        .map_err(|_| HostError::MalformedProbeField(field))?;
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(HostError::MalformedProbeField(field));
    }
    kib.checked_mul(1024).ok_or(HostError::ProbeOverflow(field))
}

fn parse_counter_field(text: &str, field: &'static str) -> Result<u64, HostError> {
    let line = text
        .lines()
        .find(|line| line.starts_with(field) && line.as_bytes().get(field.len()) == Some(&b' '))
        .ok_or(HostError::MissingProbeField(field))?;
    let mut fields = line.split_ascii_whitespace();
    if fields.next() != Some(field) {
        return Err(HostError::MalformedProbeField(field));
    }
    let value = fields
        .next()
        .ok_or(HostError::MalformedProbeField(field))?
        .parse::<u64>()
        .map_err(|_| HostError::MalformedProbeField(field))?;
    if fields.next().is_some() {
        return Err(HostError::MalformedProbeField(field));
    }
    Ok(value)
}

fn read_optional_probe(path: &str) -> Result<Option<String>, HostError> {
    let value = match std::fs::read_to_string(path) {
        Ok(value) => value,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(HostError::OptionalProbe {
                path: path.to_string(),
                source,
            });
        }
    };
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || !value.is_ascii() {
        return Err(HostError::MalformedOptionalProbe("cpufreq"));
    }
    Ok(Some(value.to_string()))
}

fn read_thermal_zones() -> Result<Vec<ThermalReading>, HostError> {
    let entries = match std::fs::read_dir("/sys/class/thermal") {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(HostError::ThermalDirectory(source)),
    };
    let mut readings = Vec::new();
    for entry in entries {
        let entry = entry.map_err(HostError::ThermalDirectory)?;
        let Some(zone) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(index) = zone.strip_prefix("thermal_zone") else {
            continue;
        };
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(HostError::MalformedThermalProbe(zone));
        }
        let index = index
            .parse::<u32>()
            .map_err(|_| HostError::MalformedThermalProbe(zone.clone()))?;
        if readings.len() == MAX_THERMAL_ZONES {
            return Err(HostError::TooManyThermalZones(MAX_THERMAL_ZONES));
        }
        let kind = read_thermal_value(&entry.path().join("type"))?;
        let temperature = read_thermal_value(&entry.path().join("temp"))?
            .parse::<i64>()
            .map_err(|_| HostError::MalformedThermalProbe(zone.clone()))?;
        if !(-273_150..=300_000).contains(&temperature) {
            return Err(HostError::MalformedThermalProbe(zone));
        }
        readings.push((
            index,
            ThermalReading {
                zone,
                kind,
                millidegrees_celsius: temperature,
            },
        ));
    }
    readings.sort_by_key(|(index, _)| *index);
    Ok(readings.into_iter().map(|(_, reading)| reading).collect())
}

fn read_thermal_value(path: &std::path::Path) -> Result<String, HostError> {
    let value = std::fs::read_to_string(path).map_err(|source| HostError::ThermalProbe {
        path: path.to_path_buf(),
        source,
    })?;
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || !value.is_ascii() {
        return Err(HostError::MalformedThermalProbe(
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(value.to_string())
}

fn cpu_identity() -> Result<String, HostError> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").map_err(HostError::CpuInfo)?;
    if cpuinfo.len() > 8 << 20 {
        return Err(HostError::CpuInfoTooLarge(cpuinfo.len()));
    }
    if let Some(model) = cpuinfo
        .lines()
        .find_map(|line| field_value(line, "model name"))
    {
        return checked_cpu_identity(model);
    }
    let first = cpuinfo.split("\n\n").next().unwrap_or(cpuinfo.as_str());
    let fields = [
        "CPU implementer",
        "CPU architecture",
        "CPU variant",
        "CPU part",
        "CPU revision",
    ];
    let identity = fields
        .iter()
        .filter_map(|field| {
            field_value(first.lines().find(|line| line.starts_with(field))?, field)
                .map(|value| format!("{field}={value}"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    checked_cpu_identity(&identity)
}

fn field_value<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let (name, value) = line.split_once(':')?;
    (name.trim() == field).then_some(value.trim())
}

fn checked_cpu_identity(value: &str) -> Result<String, HostError> {
    if value.is_empty() || value.len() > 1024 || !value.is_ascii() {
        Err(HostError::MalformedCpuIdentity)
    } else {
        Ok(value.to_string())
    }
}

fn parse_positive_finite(field: &'static str, value: &str) -> Result<f64, HostError> {
    let value = parse_nonnegative_finite(field, value)?;
    if value == 0.0 {
        Err(HostError::InvalidNumber {
            field,
            value: value.to_string(),
        })
    } else {
        Ok(value)
    }
}

fn parse_nonnegative_finite(field: &'static str, value: &str) -> Result<f64, HostError> {
    let parsed = value.parse::<f64>().map_err(|_| HostError::InvalidNumber {
        field,
        value: value.to_string(),
    })?;
    if !parsed.is_finite() || parsed < 0.0 {
        Err(HostError::InvalidNumber {
            field,
            value: value.to_string(),
        })
    } else {
        Ok(parsed)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

fn ensure_linux() -> Result<(), HostError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(HostError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
pub(crate) enum HostError {
    #[error("performance qualification host validation requires Linux")]
    UnsupportedHost,
    #[error("failed to read the source-owned host policy: {0}")]
    PolicyRead(String),
    #[error("host policy JSON is invalid: {0}")]
    PolicyJson(serde_json::Error),
    #[error("host policy schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("host policy has no profile for {operating_system}/{architecture}")]
    MissingProfile {
        operating_system: String,
        architecture: String,
    },
    #[error("host policy repeats the profile for {operating_system}/{architecture}")]
    DuplicateProfile {
        operating_system: String,
        architecture: String,
    },
    #[error("host policy profile id is invalid: {0:?}")]
    InvalidProfileId(String),
    #[error("host policy must define at least one controlled platform profile")]
    MissingProfiles,
    #[error(
        "host policy profile platform is invalid or duplicated: {operating_system}/{architecture}"
    )]
    InvalidProfilePlatform {
        operating_system: String,
        architecture: String,
    },
    #[error("host policy profile {0:?} must require positive memory and zero swap activity")]
    IncompleteProfile(String),
    #[error("host policy field {field} has invalid number {value:?}")]
    InvalidNumber { field: &'static str, value: String },
    #[error("failed to read the process CPU affinity: {0}")]
    Affinity(rustix::io::Errno),
    #[error("the qualification controller has an empty CPU affinity mask")]
    EmptyAffinity,
    #[error("failed to read /proc/loadavg: {0}")]
    LoadAverage(std::io::Error),
    #[error("/proc/loadavg is malformed")]
    MalformedLoadAverage,
    #[error("failed to read /proc/meminfo: {0}")]
    MemoryInfo(std::io::Error),
    #[error("failed to read /proc/vmstat: {0}")]
    VmStat(std::io::Error),
    #[error("failed to read /proc/cpuinfo: {0}")]
    CpuInfo(std::io::Error),
    #[error("/proc/cpuinfo is {0} bytes, exceeding the host identity limit")]
    CpuInfoTooLarge(usize),
    #[error("/proc/cpuinfo does not contain a bounded ASCII CPU identity")]
    MalformedCpuIdentity,
    #[error("failed to read optional host probe {path}: {source}")]
    OptionalProbe {
        path: String,
        source: std::io::Error,
    },
    #[error("optional host probe {0} is malformed")]
    MalformedOptionalProbe(&'static str),
    #[error("failed to enumerate thermal probes: {0}")]
    ThermalDirectory(std::io::Error),
    #[error("failed to read thermal probe {path}: {source}")]
    ThermalProbe {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("thermal probe is malformed: {0}")]
    MalformedThermalProbe(String),
    #[error("thermal probe count exceeds {0}")]
    TooManyThermalZones(usize),
    #[error("host probe is missing {0}")]
    MissingProbeField(&'static str),
    #[error("host probe field {0} is malformed")]
    MalformedProbeField(&'static str),
    #[error("host probe field {0} overflows bytes")]
    ProbeOverflow(&'static str),
    #[error("host policy rejected the qualification run: {0}")]
    PolicyViolation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_and_swap_probes_strictly() {
        assert_eq!(
            parse_kib_field("MemAvailable: 123 kB\n", MEM_AVAILABLE_FIELD).expect("valid meminfo"),
            123 * 1024
        );
        assert_eq!(
            parse_counter_field("pswpin 7\npswpout 9\n", "pswpout").expect("valid vmstat"),
            9
        );
        assert!(parse_kib_field("MemAvailable: 123 MB\n", MEM_AVAILABLE_FIELD).is_err());
        assert!(parse_counter_field("pswpin 7 extra\n", "pswpin").is_err());
    }

    #[test]
    fn strict_policy_rejects_and_local_mode_marks_violations() {
        let mut evidence = HostEvidence {
            policy_sha256: "a".repeat(64),
            profile_id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            allowed_cpus: vec![0],
            logical_cpu_count: 1,
            selected_cpu: 0,
            cpu_identity: "test CPU".to_string(),
            load_one_before: 2.0,
            load_one_after: 2.0,
            maximum_load_one: 1.0,
            available_memory_before_bytes: 1,
            available_memory_after_bytes: 1,
            minimum_available_memory_bytes: 2,
            swap_in_before: 0,
            swap_in_after: 0,
            swap_out_before: 0,
            swap_out_after: 0,
            frequency_governor_before: Some("performance".to_string()),
            frequency_governor_after: Some("performance".to_string()),
            frequency_khz_before: Some(1_000_000),
            frequency_khz_after: Some(1_000_000),
            maximum_temperature_millidegrees_celsius: 85_000,
            thermal_readings_before: Vec::new(),
            thermal_readings_after: Vec::new(),
            thermal_probe_available: false,
            verified: true,
            violations: vec!["load too high".to_string()],
        };
        assert!(enforce_or_mark_unverified(&mut evidence, false).is_err());
        assert!(enforce_or_mark_unverified(&mut evidence, true).is_ok());
        assert!(!evidence.verified);
    }

    #[test]
    fn load_memory_and_swap_activity_each_create_policy_violations() {
        let profile = HostProfile {
            id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            cpu_selection: CpuSelection::LowestAllowed,
            max_load_per_allowed_cpu: "0.50".to_string(),
            minimum_available_memory_bytes: 1024,
            require_no_swap_activity: true,
            require_frequency_governor: true,
            allowed_frequency_governors: vec!["performance".to_string()],
            require_thermal_probe: false,
            maximum_temperature_millidegrees_celsius: 85_000,
        };
        let mut evidence = HostEvidence {
            policy_sha256: "a".repeat(64),
            profile_id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            allowed_cpus: vec![0],
            logical_cpu_count: 1,
            selected_cpu: 0,
            cpu_identity: "test CPU".to_string(),
            load_one_before: 2.0,
            load_one_after: 2.0,
            maximum_load_one: 0.5,
            available_memory_before_bytes: 512,
            available_memory_after_bytes: 512,
            minimum_available_memory_bytes: 1024,
            swap_in_before: 1,
            swap_in_after: 2,
            swap_out_before: 3,
            swap_out_after: 4,
            frequency_governor_before: Some("performance".to_string()),
            frequency_governor_after: Some("performance".to_string()),
            frequency_khz_before: Some(1_000_000),
            frequency_khz_after: Some(1_000_000),
            maximum_temperature_millidegrees_celsius: 85_000,
            thermal_readings_before: Vec::new(),
            thermal_readings_after: Vec::new(),
            thermal_probe_available: false,
            verified: true,
            violations: Vec::new(),
        };
        let snapshot = HostSnapshot {
            load_one: 2.0,
            available_memory_bytes: 512,
            swap_in: 2,
            swap_out: 4,
            frequency_governor: Some("performance".to_string()),
            frequency_khz: Some(1_000_000),
            thermal_readings: Vec::new(),
        };

        append_snapshot_violations(&profile, &mut evidence, &snapshot, true);
        append_swap_violation(&profile, &mut evidence, &snapshot);

        assert_eq!(evidence.violations.len(), 3);
        assert!(enforce_or_mark_unverified(&mut evidence, false).is_err());
    }

    #[test]
    fn required_frequency_probe_must_be_available_and_allowed() {
        let profile = HostProfile {
            id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            cpu_selection: CpuSelection::LowestAllowed,
            max_load_per_allowed_cpu: "0.50".to_string(),
            minimum_available_memory_bytes: 1024,
            require_no_swap_activity: true,
            require_frequency_governor: true,
            allowed_frequency_governors: vec!["performance".to_string()],
            require_thermal_probe: false,
            maximum_temperature_millidegrees_celsius: 85_000,
        };
        let mut evidence = HostEvidence {
            policy_sha256: "a".repeat(64),
            profile_id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            allowed_cpus: vec![0],
            logical_cpu_count: 1,
            selected_cpu: 0,
            cpu_identity: "test CPU".to_string(),
            load_one_before: 0.0,
            load_one_after: 0.0,
            maximum_load_one: 0.5,
            available_memory_before_bytes: 2048,
            available_memory_after_bytes: 2048,
            minimum_available_memory_bytes: 1024,
            swap_in_before: 0,
            swap_in_after: 0,
            swap_out_before: 0,
            swap_out_after: 0,
            frequency_governor_before: None,
            frequency_governor_after: None,
            frequency_khz_before: None,
            frequency_khz_after: None,
            maximum_temperature_millidegrees_celsius: 85_000,
            thermal_readings_before: Vec::new(),
            thermal_readings_after: Vec::new(),
            thermal_probe_available: false,
            verified: true,
            violations: Vec::new(),
        };
        let unavailable = HostSnapshot {
            load_one: 0.0,
            available_memory_bytes: 2048,
            swap_in: 0,
            swap_out: 0,
            frequency_governor: None,
            frequency_khz: None,
            thermal_readings: Vec::new(),
        };
        append_snapshot_violations(&profile, &mut evidence, &unavailable, true);
        assert!(
            evidence
                .violations
                .iter()
                .any(|value| value.contains("unavailable"))
        );

        evidence.violations.clear();
        let disallowed = HostSnapshot {
            frequency_governor: Some("powersave".to_string()),
            ..unavailable
        };
        append_snapshot_violations(&profile, &mut evidence, &disallowed, false);
        assert!(
            evidence
                .violations
                .iter()
                .any(|value| value.contains("powersave"))
        );
    }

    #[test]
    fn required_thermal_probe_must_stay_available_stable_and_below_limit() {
        let profile = HostProfile {
            id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            cpu_selection: CpuSelection::LowestAllowed,
            max_load_per_allowed_cpu: "0.50".to_string(),
            minimum_available_memory_bytes: 1024,
            require_no_swap_activity: true,
            require_frequency_governor: false,
            allowed_frequency_governors: Vec::new(),
            require_thermal_probe: true,
            maximum_temperature_millidegrees_celsius: 85_000,
        };
        let baseline = ThermalReading {
            zone: "thermal_zone0".to_string(),
            kind: "cpu".to_string(),
            millidegrees_celsius: 45_000,
        };
        let mut evidence = HostEvidence {
            policy_sha256: "a".repeat(64),
            profile_id: "test".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
            allowed_cpus: vec![0],
            logical_cpu_count: 1,
            selected_cpu: 0,
            cpu_identity: "test CPU".to_string(),
            load_one_before: 0.0,
            load_one_after: 0.0,
            maximum_load_one: 0.5,
            available_memory_before_bytes: 2048,
            available_memory_after_bytes: 2048,
            minimum_available_memory_bytes: 1024,
            swap_in_before: 0,
            swap_in_after: 0,
            swap_out_before: 0,
            swap_out_after: 0,
            frequency_governor_before: None,
            frequency_governor_after: None,
            frequency_khz_before: None,
            frequency_khz_after: None,
            maximum_temperature_millidegrees_celsius: 85_000,
            thermal_readings_before: vec![baseline.clone()],
            thermal_readings_after: vec![baseline],
            thermal_probe_available: true,
            verified: true,
            violations: Vec::new(),
        };
        let unavailable = HostSnapshot {
            load_one: 0.0,
            available_memory_bytes: 2048,
            swap_in: 0,
            swap_out: 0,
            frequency_governor: None,
            frequency_khz: None,
            thermal_readings: Vec::new(),
        };
        append_snapshot_violations(&profile, &mut evidence, &unavailable, false);
        assert!(
            evidence
                .violations
                .iter()
                .any(|value| value.contains("unavailable"))
        );
        assert!(
            evidence
                .violations
                .iter()
                .any(|value| value.contains("zone set changed"))
        );

        evidence.violations.clear();
        let overheated = HostSnapshot {
            thermal_readings: vec![ThermalReading {
                zone: "thermal_zone0".to_string(),
                kind: "cpu".to_string(),
                millidegrees_celsius: 90_000,
            }],
            ..unavailable
        };
        append_snapshot_violations(&profile, &mut evidence, &overheated, false);
        assert!(
            evidence
                .violations
                .iter()
                .any(|value| value.contains("exceeding 85000"))
        );
        assert!(
            !evidence
                .violations
                .iter()
                .any(|value| value.contains("zone set changed"))
        );
    }
}
