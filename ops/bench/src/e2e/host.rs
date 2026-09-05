use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HostProfile {
    pub(super) architecture: String,
    pub(super) cpu_model: String,
    pub(super) logical_cpus: usize,
    pub(super) affinity_cpu: Option<usize>,
    pub(super) kernel_release: String,
    pub(super) thermal: Vec<ThermalReading>,
    pub(super) swap: SwapSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ThermalReading {
    pub(super) zone: String,
    pub(super) kind: String,
    pub(super) millidegrees: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SwapSnapshot {
    pub(super) configured: Vec<String>,
    pub(super) pages_in: u64,
    pub(super) pages_out: u64,
}

impl HostProfile {
    pub(super) fn capture(affinity_cpu: Option<usize>) -> Result<Self, String> {
        let logical_cpus = std::thread::available_parallelism()
            .map_err(|source| format!("cannot determine logical CPU count: {source}"))?
            .get();
        Ok(Self {
            architecture: std::env::consts::ARCH.to_string(),
            cpu_model: cpu_model()?,
            logical_cpus,
            affinity_cpu,
            kernel_release: read_trimmed(Path::new("/proc/sys/kernel/osrelease"))?,
            thermal: thermal_readings()?,
            swap: swap_snapshot()?,
        })
    }

    pub(super) fn validate_formal(
        &self,
        maximum_temperature_millidegrees: u64,
    ) -> Result<(), String> {
        validate_recorded_host_transition(self, self, maximum_temperature_millidegrees, true)?;
        let competitors = competing_benchmark_processes()?;
        if !competitors.is_empty() {
            return Err(format!(
                "competing benchmark processes are active: {}",
                competitors.join(", ")
            ));
        }
        Ok(())
    }
}

pub(super) fn validate_host_after(
    before: &HostProfile,
    after: &HostProfile,
    maximum_temperature_millidegrees: u64,
    formal: bool,
) -> Result<(), String> {
    validate_recorded_host_transition(before, after, maximum_temperature_millidegrees, formal)?;
    if formal {
        let competitors = competing_benchmark_processes()?;
        if !competitors.is_empty() {
            return Err(format!(
                "competing benchmark processes are active: {}",
                competitors.join(", ")
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_recorded_host_transition(
    before: &HostProfile,
    after: &HostProfile,
    maximum_temperature_millidegrees: u64,
    formal: bool,
) -> Result<(), String> {
    if before.architecture != after.architecture
        || before.cpu_model != after.cpu_model
        || before.logical_cpus != after.logical_cpus
        || before.affinity_cpu != after.affinity_cpu
        || before.kernel_release != after.kernel_release
        || before.swap.configured != after.swap.configured
    {
        return Err("host identity or swap configuration changed during the run".to_string());
    }
    if formal
        && (after.swap.pages_in != before.swap.pages_in
            || after.swap.pages_out != before.swap.pages_out)
    {
        return Err(format!(
            "swap I/O changed during timing: in {}->{}, out {}->{}",
            before.swap.pages_in, after.swap.pages_in, before.swap.pages_out, after.swap.pages_out
        ));
    }
    if formal {
        for profile in [before, after] {
            if profile.affinity_cpu.is_none() {
                return Err("formal timing requires an explicit affinity CPU".to_string());
            }
            if profile.thermal.is_empty() {
                return Err("formal timing requires at least one readable thermal zone".to_string());
            }
            if let Some(reading) = profile
                .thermal
                .iter()
                .find(|reading| reading.millidegrees > maximum_temperature_millidegrees)
            {
                return Err(format!(
                    "thermal zone {} is {} millidegrees, above {}",
                    reading.zone, reading.millidegrees, maximum_temperature_millidegrees
                ));
            }
        }
    }
    Ok(())
}

fn cpu_model() -> Result<String, String> {
    let text = fs::read_to_string("/proc/cpuinfo")
        .map_err(|source| format!("cannot read /proc/cpuinfo: {source}"))?;
    for key in ["Model", "model name", "Hardware"] {
        if let Some(value) = text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_string())
        }) && !value.is_empty()
        {
            return Ok(value);
        }
    }
    if let Ok(product) = read_trimmed(Path::new("/sys/devices/virtual/dmi/id/product_name"))
        && !product.is_empty()
    {
        let mut parts = text
            .lines()
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                (name.trim() == "CPU part").then(|| value.trim().to_string())
            })
            .collect::<Vec<_>>();
        parts.sort();
        parts.dedup();
        return Ok(if parts.is_empty() {
            product
        } else {
            format!("{product} ({})", parts.join(","))
        });
    }
    Err("host exposes neither a CPU model nor a DMI product name".to_string())
}

fn thermal_readings() -> Result<Vec<ThermalReading>, String> {
    let root = Path::new("/sys/class/thermal");
    let mut zones = fs::read_dir(root)
        .map_err(|source| format!("cannot read {}: {source}", root.display()))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("thermal_zone")
        })
        .collect::<Vec<_>>();
    zones.sort_by_key(|entry| entry.file_name());
    let mut readings = Vec::new();
    for zone in zones {
        let temp = match read_trimmed(&zone.path().join("temp")) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Ok(millidegrees) = temp.parse::<u64>() else {
            continue;
        };
        readings.push(ThermalReading {
            zone: zone.file_name().to_string_lossy().into_owned(),
            kind: read_trimmed(&zone.path().join("type")).unwrap_or_else(|_| "unknown".to_string()),
            millidegrees,
        });
    }
    Ok(readings)
}

fn swap_snapshot() -> Result<SwapSnapshot, String> {
    let swaps = fs::read_to_string("/proc/swaps")
        .map_err(|source| format!("cannot read /proc/swaps: {source}"))?;
    let configured = swaps
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    let vmstat = fs::read_to_string("/proc/vmstat")
        .map_err(|source| format!("cannot read /proc/vmstat: {source}"))?;
    let mut pages_in = None;
    let mut pages_out = None;
    for line in vmstat.lines() {
        let Some((name, value)) = line.split_once(' ') else {
            continue;
        };
        match name {
            "pswpin" => pages_in = value.trim().parse::<u64>().ok(),
            "pswpout" => pages_out = value.trim().parse::<u64>().ok(),
            _ => {}
        }
    }
    Ok(SwapSnapshot {
        configured,
        pages_in: pages_in.ok_or_else(|| "/proc/vmstat has no pswpin".to_string())?,
        pages_out: pages_out.ok_or_else(|| "/proc/vmstat has no pswpout".to_string())?,
    })
}

fn competing_benchmark_processes() -> Result<Vec<String>, String> {
    let own_pid = std::process::id();
    let ancestors = ancestor_pids(own_pid)?;
    let mut matches = Vec::new();
    for entry in fs::read_dir("/proc")
        .map_err(|source| format!("cannot scan /proc: {source}"))?
        .filter_map(Result::ok)
    {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        if pid == own_pid || ancestors.contains(&pid) {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let command = String::from_utf8_lossy(&bytes).replace('\0', " ");
        if command.contains("stab-bench")
            && (command.contains("e2e-run") || command.contains("e2e-worker"))
        {
            matches.push(format!("{pid}:{command}"));
        }
    }
    matches.sort();
    Ok(matches)
}

fn ancestor_pids(pid: u32) -> Result<BTreeSet<u32>, String> {
    let mut ancestors = BTreeSet::new();
    let mut current = pid;
    for _ in 0..256 {
        let Some(parent) = process_parent_pid(current)? else {
            break;
        };
        if parent == 0 || !ancestors.insert(parent) {
            break;
        }
        current = parent;
    }
    Ok(ancestors)
}

fn process_parent_pid(pid: u32) -> Result<Option<u32>, String> {
    let path = Path::new("/proc").join(pid.to_string()).join("stat");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(format!("cannot read {}: {source}", path.display())),
    };
    let (_, suffix) = text
        .rsplit_once(')')
        .ok_or_else(|| format!("{} has malformed process stat data", path.display()))?;
    let mut fields = suffix.split_whitespace();
    fields
        .next()
        .ok_or_else(|| format!("{} omits process state", path.display()))?;
    let parent = fields
        .next()
        .ok_or_else(|| format!("{} omits parent process id", path.display()))?
        .parse::<u32>()
        .map_err(|source| format!("{} has invalid parent process id: {source}", path.display()))?;
    Ok(Some(parent))
}

fn read_trimmed(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|value| value.trim().to_string())
        .map_err(|source| format!("cannot read {}: {source}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    fn affinity_request(cpu: usize, child: bool) -> crate::process::ProcessRequest {
        use std::ffi::OsString;
        use std::time::Duration;

        crate::process::ProcessRequest {
            program: std::env::current_exe().expect("test executable"),
            args: [
                "e2e::host::tests::restricted_affinity_helper",
                "--exact",
                "--ignored",
                "--nocapture",
            ]
            .map(OsString::from)
            .into(),
            stdin: vec![b'\n'],
            working_directory: std::env::current_dir().expect("working directory"),
            environment: vec![
                (OsString::from("STAB_HOST_TEST_CPU"), cpu.to_string().into()),
                (
                    OsString::from("STAB_HOST_TEST_CHILD"),
                    child.to_string().into(),
                ),
            ]
            .into(),
            affinity_cpu: child.then_some(cpu),
            limits: crate::process::ProcessLimits {
                stdin_bytes: 1,
                stdout: 4096.into(),
                stderr: 4096.into(),
                regular_file_bytes: None,
                timeout: Duration::from_secs(10),
            },
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn host_capture_accepts_a_restricted_nonzero_cpu() {
        let allowed = rustix::thread::sched_getaffinity(None).expect("read affinity");
        let Some(cpu) = (1..rustix::thread::CpuSet::MAX_CPU).find(|cpu| allowed.is_set(*cpu))
        else {
            eprintln!("no nonzero CPU is available for the restricted-affinity regression");
            return;
        };
        let output = crate::process::run_bounded_process(&affinity_request(cpu, false))
            .expect("restricted controller completes");
        assert_eq!(
            output.status,
            Some(0),
            "restricted controller stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "runs in a subprocess so affinity changes cannot affect other tests"]
    fn restricted_affinity_helper() {
        use std::io::Read as _;

        let cpu = std::env::var("STAB_HOST_TEST_CPU")
            .expect("requested CPU")
            .parse::<usize>()
            .expect("numeric CPU");
        let mut expected = rustix::thread::CpuSet::new();
        expected.set(cpu);
        if std::env::var("STAB_HOST_TEST_CHILD").as_deref() == Ok("true") {
            std::io::stdin()
                .read_exact(&mut [0])
                .expect("affinity handshake");
            let actual = rustix::thread::sched_getaffinity(None).expect("child affinity");
            assert_eq!(actual, expected);
            return;
        }
        rustix::thread::sched_setaffinity(None, &expected).expect("restrict controller affinity");
        let profile = HostProfile::capture(Some(cpu)).expect("capture restricted host");
        let output = crate::process::run_bounded_process(&affinity_request(
            profile.affinity_cpu.expect("selected CPU"),
            true,
        ))
        .expect("supervised child completes");
        assert_eq!(
            output.status,
            Some(0),
            "affinity child stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn host_after_rejects_swap_io_and_identity_changes() {
        let profile = HostProfile {
            architecture: "aarch64".to_string(),
            cpu_model: "test".to_string(),
            logical_cpus: 4,
            affinity_cpu: Some(2),
            kernel_release: "test".to_string(),
            thermal: vec![ThermalReading {
                zone: "zone0".to_string(),
                kind: "cpu".to_string(),
                millidegrees: 50_000,
            }],
            swap: SwapSnapshot {
                configured: vec!["/swap".to_string()],
                pages_in: 10,
                pages_out: 20,
            },
        };
        assert_eq!(
            validate_host_after(&profile, &profile, 100_000, false),
            Ok(())
        );

        let mut changed = profile.clone();
        changed.swap.pages_out += 1;
        assert!(validate_host_after(&profile, &changed, 100_000, true).is_err());
    }

    #[test]
    fn formal_host_requires_affinity_temperature_and_ceiling() {
        let profile = HostProfile {
            architecture: "aarch64".to_string(),
            cpu_model: "test".to_string(),
            logical_cpus: 4,
            affinity_cpu: None,
            kernel_release: "test".to_string(),
            thermal: Vec::new(),
            swap: SwapSnapshot {
                configured: Vec::new(),
                pages_in: 0,
                pages_out: 0,
            },
        };
        assert!(profile.validate_formal(100_000).is_err());
    }

    #[test]
    fn benchmark_process_scan_excludes_the_controller_ancestor_chain() {
        let ancestors = ancestor_pids(std::process::id()).expect("process ancestors");
        assert!(!ancestors.is_empty());
        assert!(!ancestors.contains(&std::process::id()));
    }
}
