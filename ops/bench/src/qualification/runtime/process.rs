use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use thiserror::Error;

const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessLimits {
    pub(crate) stdin_bytes: usize,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) regular_file_bytes: Option<u64>,
    pub(crate) timeout: Duration,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessRequest {
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<OsString>,
    pub(crate) stdin: Vec<u8>,
    pub(crate) working_directory: PathBuf,
    pub(crate) environment: Vec<(OsString, OsString)>,
    pub(crate) affinity_cpu: Option<usize>,
    pub(crate) limits: ProcessLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessResult {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) parent_observed_peak_rss_bytes: Option<u64>,
    pub(crate) wall_elapsed: Duration,
}

pub(crate) fn run_bounded_process(request: &ProcessRequest) -> Result<ProcessResult, ProcessError> {
    let wall_started = Instant::now();
    ensure_linux()?;
    if request.stdin.len() > request.limits.stdin_bytes {
        return Err(ProcessError::StdinLimit {
            actual: request.stdin.len(),
            maximum: request.limits.stdin_bytes,
        });
    }
    if request.limits.timeout.is_zero() {
        return Err(ProcessError::ZeroTimeout);
    }
    if request.limits.regular_file_bytes == Some(0) {
        return Err(ProcessError::ZeroFileLimit);
    }
    let cancellation = ProcessCancellation::for_signals()?;
    if cancellation.is_cancelled() {
        return Err(ProcessError::Interrupted {
            program: request.program.clone(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }

    let mut command = std::process::Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.working_directory)
        .env_clear()
        .envs(request.environment.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
        program: request.program.clone(),
        source,
    })?;
    if let Some(cpu) = request.affinity_cpu
        && let Err(source) = set_child_affinity(child.id(), cpu)
    {
        terminate_process_tree(&mut child, &request.program, false)?;
        return Err(ProcessError::SetAffinity {
            program: request.program.clone(),
            cpu,
            source,
        });
    }
    if let Some(maximum) = request.limits.regular_file_bytes
        && let Err(source) = set_child_file_limit(child.id(), maximum)
    {
        terminate_process_tree(&mut child, &request.program, false)?;
        return Err(ProcessError::SetFileLimit {
            program: request.program.clone(),
            maximum,
            source,
        });
    }
    let pid = child.id();
    let deadline = Instant::now()
        .checked_add(request.limits.timeout)
        .ok_or(ProcessError::DeadlineOverflow)?;
    let stdout = child
        .stdout
        .take()
        .map(|pipe| spawn_reader(pipe, request.limits.stdout_bytes))
        .ok_or(ProcessError::MissingPipe("stdout"))?;
    let stderr = child
        .stderr
        .take()
        .map(|pipe| spawn_reader(pipe, request.limits.stderr_bytes))
        .ok_or(ProcessError::MissingPipe("stderr"))?;
    let stdin = child
        .stdin
        .take()
        .map(|pipe| spawn_writer(pipe, request.stdin.clone()))
        .ok_or(ProcessError::MissingPipe("stdin"))?;
    let mut status = None;
    let mut peak_rss = None;

    loop {
        if let Some(rss) = process_rss_bytes(pid)? {
            peak_rss = Some(peak_rss.map_or(rss, |peak: u64| peak.max(rss)));
        }
        if status.is_none() {
            status = child.try_wait().map_err(|source| ProcessError::Wait {
                program: request.program.clone(),
                source,
            })?;
            if status.is_some() {
                kill_process_group(&mut child, &request.program)?;
            }
        }
        if cancellation.is_cancelled() {
            terminate_process_tree(&mut child, &request.program, status.is_some())?;
            let captured = join_all(&request.program, stdin, stdout, stderr)?;
            return Err(ProcessError::Interrupted {
                program: request.program.clone(),
                stdout: captured.stdout,
                stderr: captured.stderr,
            });
        }
        if stdout.exceeded() || stderr.exceeded() {
            let stream = if stdout.exceeded() {
                "stdout"
            } else {
                "stderr"
            };
            let maximum = if stream == "stdout" {
                request.limits.stdout_bytes
            } else {
                request.limits.stderr_bytes
            };
            terminate_process_tree(&mut child, &request.program, status.is_some())?;
            let captured = join_all(&request.program, stdin, stdout, stderr)?;
            return Err(ProcessError::OutputLimit {
                program: request.program.clone(),
                stream,
                maximum,
                stdout: captured.stdout,
                stderr: captured.stderr,
            });
        }
        if status.is_some() && stdin.is_finished() && stdout.is_finished() && stderr.is_finished() {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            terminate_process_tree(&mut child, &request.program, status.is_some())?;
            let captured = join_all(&request.program, stdin, stdout, stderr)?;
            return Err(ProcessError::TimedOut {
                program: request.program.clone(),
                timeout: request.limits.timeout,
                stdout: captured.stdout,
                stderr: captured.stderr,
            });
        }
        std::thread::sleep(POLL_INTERVAL.min(deadline.duration_since(now)));
    }

    join_writer(&request.program, stdin)?;
    let stdout = join_reader(&request.program, stdout)?;
    let stderr = join_reader(&request.program, stderr)?;
    let status = status.ok_or_else(|| ProcessError::MissingStatus(request.program.clone()))?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(maximum) = request.limits.regular_file_bytes
            && status.signal() == Some(signal_hook::consts::signal::SIGXFSZ)
        {
            return Err(ProcessError::FileLimit {
                program: request.program.clone(),
                maximum,
                stdout: stdout.bytes,
                stderr: stderr.bytes,
            });
        }
    }
    Ok(ProcessResult {
        status: status.code(),
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        parent_observed_peak_rss_bytes: peak_rss,
        wall_elapsed: wall_started.elapsed(),
    })
}

struct JoinedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn join_all(
    program: &std::path::Path,
    stdin: Writer,
    stdout: OutputReader,
    stderr: OutputReader,
) -> Result<JoinedOutput, ProcessError> {
    let writer = join_writer(program, stdin);
    let stdout = join_reader(program, stdout)?;
    let stderr = join_reader(program, stderr)?;
    writer?;
    Ok(JoinedOutput {
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

struct ProcessCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProcessCancellation {
    fn for_signals() -> Result<Self, ProcessError> {
        static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
        static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
        let cancelled = Arc::clone(CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false))));
        let installed = INSTALLED.get_or_init(|| {
            for signal in [
                signal_hook::consts::signal::SIGINT,
                signal_hook::consts::signal::SIGTERM,
            ] {
                signal_hook::flag::register(signal, Arc::clone(&cancelled))
                    .map_err(|source| source.to_string())?;
            }
            Ok(())
        });
        if let Err(reason) = installed {
            return Err(ProcessError::InstallSignals(reason.clone()));
        }
        Ok(Self { cancelled })
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

type Writer = JoinHandle<Result<(), std::io::Error>>;

fn spawn_writer(mut pipe: std::process::ChildStdin, bytes: Vec<u8>) -> Writer {
    std::thread::spawn(move || pipe.write_all(&bytes))
}

fn join_writer(program: &std::path::Path, writer: Writer) -> Result<(), ProcessError> {
    match writer.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(ProcessError::WriteStdin {
            program: program.to_path_buf(),
            source,
        }),
        Err(_) => Err(ProcessError::WriteStdin {
            program: program.to_path_buf(),
            source: std::io::Error::other("stdin writer thread panicked"),
        }),
    }
}

struct CapturedOutput {
    bytes: Vec<u8>,
}

struct OutputReader {
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
    exceeded: Arc<AtomicBool>,
}

impl OutputReader {
    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

fn spawn_reader(mut pipe: impl Read + Send + 'static, maximum: usize) -> OutputReader {
    let exceeded = Arc::new(AtomicBool::new(false));
    let reader_exceeded = Arc::clone(&exceeded);
    let handle = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = pipe.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let chunk = buffer
                .get(..count)
                .ok_or_else(|| std::io::Error::other("pipe read exceeded buffer bounds"))?;
            let remaining = maximum.saturating_sub(bytes.len());
            if count <= remaining {
                bytes.extend_from_slice(chunk);
            } else {
                let kept = chunk
                    .get(..remaining)
                    .ok_or_else(|| std::io::Error::other("pipe keep exceeded buffer bounds"))?;
                bytes.extend_from_slice(kept);
                reader_exceeded.store(true, Ordering::Release);
            }
        }
        Ok(CapturedOutput { bytes })
    });
    OutputReader { handle, exceeded }
}

fn join_reader(
    program: &std::path::Path,
    reader: OutputReader,
) -> Result<CapturedOutput, ProcessError> {
    match reader.handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(ProcessError::Capture {
            program: program.to_path_buf(),
            source,
        }),
        Err(_) => Err(ProcessError::Capture {
            program: program.to_path_buf(),
            source: std::io::Error::other("output reader thread panicked"),
        }),
    }
}

fn terminate_process_tree(
    child: &mut std::process::Child,
    program: &std::path::Path,
    already_reaped: bool,
) -> Result<(), ProcessError> {
    kill_process_group(child, program)?;
    if !already_reaped {
        child.wait().map_err(|source| ProcessError::Wait {
            program: program.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn kill_process_group(
    child: &mut std::process::Child,
    program: &std::path::Path,
) -> Result<(), ProcessError> {
    let raw_pid = i32::try_from(child.id()).map_err(|_| ProcessError::InvalidPid(child.id()))?;
    let pid =
        rustix::process::Pid::from_raw(raw_pid).ok_or(ProcessError::InvalidPid(child.id()))?;
    if let Err(source) = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
        && source != rustix::io::Errno::SRCH
    {
        return Err(ProcessError::Kill {
            program: program.to_path_buf(),
            source: source.into(),
        });
    }
    Ok(())
}

fn set_child_affinity(pid: u32, cpu: usize) -> Result<(), std::io::Error> {
    if cpu >= rustix::thread::CpuSet::MAX_CPU {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("CPU {cpu} exceeds the supported affinity mask"),
        ));
    }
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })?;
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu);
    rustix::thread::sched_setaffinity(Some(pid), &set).map_err(Into::into)
}

fn set_child_file_limit(pid: u32, requested_maximum: u64) -> Result<(), std::io::Error> {
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })?;
    let inherited = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    let maximum = inherited
        .maximum
        .map_or(requested_maximum, |hard| hard.min(requested_maximum));
    rustix::process::prlimit(
        Some(pid),
        rustix::process::Resource::Fsize,
        rustix::process::Rlimit {
            current: Some(maximum),
            maximum: Some(maximum),
        },
    )
    .map(|_| ())
    .map_err(Into::into)
}

fn process_rss_bytes(pid: u32) -> Result<Option<u64>, ProcessError> {
    let path = PathBuf::from(format!("/proc/{pid}/status"));
    let status = match std::fs::read_to_string(&path) {
        Ok(status) => status,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ProcessError::ReadRss { path, source }),
    };
    let Some(line) = status.lines().find(|line| line.starts_with("VmRSS:")) else {
        // A reaped or zombie process can retain /proc/<pid>/status after VmRSS disappears.
        return Ok(None);
    };
    let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
    let ["VmRSS:", kib, "kB"] = fields.as_slice() else {
        return Err(ProcessError::MalformedRss(path));
    };
    let kib = kib
        .parse::<u64>()
        .map_err(|_| ProcessError::MalformedRss(path.clone()))?;
    kib.checked_mul(1024)
        .map(Some)
        .ok_or(ProcessError::RssOverflow(path))
}

fn ensure_linux() -> Result<(), ProcessError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(ProcessError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ProcessError {
    #[error("performance qualification process control requires Linux")]
    UnsupportedHost,
    #[error("process stdin is {actual} bytes, exceeding {maximum}")]
    StdinLimit { actual: usize, maximum: usize },
    #[error("process timeout must be positive")]
    ZeroTimeout,
    #[error("process regular-file limit must be positive when configured")]
    ZeroFileLimit,
    #[error("process timeout exceeds the monotonic clock range")]
    DeadlineOverflow,
    #[error("failed to install qualification signal handlers: {0}")]
    InstallSignals(String),
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to pin {program} to CPU {cpu}: {source}")]
    SetAffinity {
        program: PathBuf,
        cpu: usize,
        source: std::io::Error,
    },
    #[error("failed to limit regular files written by {program} to {maximum} bytes: {source}")]
    SetFileLimit {
        program: PathBuf,
        maximum: u64,
        source: std::io::Error,
    },
    #[error("spawned process is missing its piped {0}")]
    MissingPipe(&'static str),
    #[error("failed to wait for {program}: {source}")]
    Wait {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write stdin for {program}: {source}")]
    WriteStdin {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to capture output from {program}: {source}")]
    Capture {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("{program} {stream} exceeded {maximum} bytes")]
    OutputLimit {
        program: PathBuf,
        stream: &'static str,
        maximum: usize,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("{program} timed out after {timeout:?}")]
    TimedOut {
        program: PathBuf,
        timeout: Duration,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("{program} exceeded the regular-file limit of {maximum} bytes")]
    FileLimit {
        program: PathBuf,
        maximum: u64,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("{program} was interrupted")]
    Interrupted {
        program: PathBuf,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("child process id {0} exceeds the supported Linux pid range")]
    InvalidPid(u32),
    #[error("{0} completed without an observable exit status")]
    MissingStatus(PathBuf),
    #[error("failed to terminate the process group for {program}: {source}")]
    Kill {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read process RSS from {path}: {source}")]
    ReadRss {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("process RSS in {0} is malformed")]
    MalformedRss(PathBuf),
    #[error("process RSS in {0} overflows u64 bytes")]
    RssOverflow(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELPER_TEST: &str = "qualification::runtime::process::tests::process_helper";
    const HELPER_ENV: &str = "STAB_BENCH_PROCESS_HELPER";
    const EXPECTED_CPU_ENV: &str = "STAB_BENCH_EXPECTED_CPU";
    const OUTPUT_PATH_ENV: &str = "STAB_BENCH_OUTPUT_PATH";

    fn request(mode: &str) -> ProcessRequest {
        ProcessRequest {
            program: std::env::current_exe().expect("test executable"),
            args: vec![
                OsString::from(HELPER_TEST),
                OsString::from("--exact"),
                OsString::from("--ignored"),
                OsString::from("--nocapture"),
            ],
            stdin: Vec::new(),
            working_directory: std::env::current_dir().expect("working directory"),
            environment: vec![(OsString::from(HELPER_ENV), OsString::from(mode))],
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 8 << 20,
                stdout_bytes: 4096,
                stderr_bytes: 4096,
                regular_file_bytes: None,
                timeout: Duration::from_secs(2),
            },
        }
    }

    #[test]
    fn captures_success_nonzero_and_signal_status() {
        let success = run_bounded_process(&request("success")).expect("successful helper");
        assert_eq!(success.status, Some(0));
        assert!(String::from_utf8_lossy(&success.stdout).contains("helper-success"));

        let nonzero = run_bounded_process(&request("nonzero")).expect("nonzero is captured");
        assert_eq!(nonzero.status, Some(7));

        let signalled = run_bounded_process(&request("signal")).expect("signal is captured");
        assert_eq!(signalled.status, None);
    }

    #[test]
    fn rejects_missing_binary_and_all_stream_limits() {
        let mut missing = request("success");
        missing.program = PathBuf::from("/definitely/missing/stab-bench-worker");
        assert!(matches!(
            run_bounded_process(&missing),
            Err(ProcessError::Spawn { .. })
        ));

        let mut stdin = request("success");
        stdin.stdin = vec![0_u8; 2];
        stdin.limits.stdin_bytes = 1;
        assert!(matches!(
            run_bounded_process(&stdin),
            Err(ProcessError::StdinLimit { .. })
        ));

        let mut stdout = request("stdout-overflow");
        stdout.limits.stdout_bytes = 32;
        assert!(matches!(
            run_bounded_process(&stdout),
            Err(ProcessError::OutputLimit {
                stream: "stdout",
                ..
            })
        ));

        let mut stderr = request("stderr-overflow");
        stderr.limits.stderr_bytes = 32;
        assert!(matches!(
            run_bounded_process(&stderr),
            Err(ProcessError::OutputLimit {
                stream: "stderr",
                ..
            })
        ));
    }

    #[test]
    fn propagates_writer_failure() {
        let mut request = request("close-stdin");
        request.stdin = vec![0_u8; 8 << 20];
        let result = run_bounded_process(&request);
        assert!(matches!(result, Err(ProcessError::WriteStdin { .. })));
    }

    #[test]
    fn pins_the_child_to_the_requested_singleton_cpu() {
        let allowed = rustix::thread::sched_getaffinity(None).expect("read parent affinity");
        let cpu = (0..rustix::thread::CpuSet::MAX_CPU)
            .find(|cpu| allowed.is_set(*cpu))
            .expect("at least one allowed CPU");
        let mut request = request("affinity");
        request.affinity_cpu = Some(cpu);
        request.environment.push((
            OsString::from(EXPECTED_CPU_ENV),
            OsString::from(cpu.to_string()),
        ));

        let result = run_bounded_process(&request).expect("affinity helper succeeds");

        assert_eq!(result.status, Some(0));
        assert!(String::from_utf8_lossy(&result.stdout).contains("affinity-ok"));
    }

    #[test]
    fn rejects_invalid_affinity_and_bounds_regular_files() {
        let mut invalid_affinity = request("success");
        invalid_affinity.affinity_cpu = Some(rustix::thread::CpuSet::MAX_CPU);
        assert!(matches!(
            run_bounded_process(&invalid_affinity),
            Err(ProcessError::SetAffinity { .. })
        ));

        let directory = tempfile::tempdir().expect("temporary output directory");
        let output = directory.path().join("bounded-output");
        let mut bounded_file = request("file-overflow");
        bounded_file.stdin = vec![b'\n'];
        bounded_file.limits.stdin_bytes = 1;
        bounded_file.limits.regular_file_bytes = Some(64);
        bounded_file.environment.push((
            OsString::from(OUTPUT_PATH_ENV),
            output.as_os_str().to_os_string(),
        ));
        assert!(matches!(
            run_bounded_process(&bounded_file),
            Err(ProcessError::FileLimit { maximum: 64, .. })
        ));
        assert!(
            std::fs::metadata(output)
                .expect("bounded output exists")
                .len()
                <= 64
        );
    }

    #[test]
    fn captures_worker_panic_as_a_failed_process() {
        let output = run_bounded_process(&request("panic")).expect("panic is captured");
        assert_eq!(output.status, Some(101));
        assert!(String::from_utf8_lossy(&output.stderr).contains("process helper panic"));
    }

    #[test]
    fn timeout_kills_the_entire_process_group() {
        let mut request = request("child-tree");
        request.limits.timeout = Duration::from_millis(100);
        let error = run_bounded_process(&request).expect_err("helper must time out");
        assert!(matches!(error, ProcessError::TimedOut { .. }));
        let ProcessError::TimedOut { stdout, .. } = error else {
            unreachable!("timeout shape checked above");
        };
        let output = String::from_utf8_lossy(&stdout);
        let pid = output
            .lines()
            .find_map(|line| line.strip_prefix("grandchild-pid="))
            .expect("grandchild pid")
            .parse::<u32>()
            .expect("numeric grandchild pid");
        for _ in 0..100 {
            if !PathBuf::from(format!("/proc/{pid}")).exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "grandchild process {pid} survived process-group timeout"
        );
    }

    #[test]
    #[ignore = "executed only as a subprocess by bounded-process tests"]
    fn process_helper() {
        let mode = std::env::var(HELPER_ENV).expect("helper mode");
        match mode.as_str() {
            "success" => println!("helper-success"),
            "nonzero" => std::process::exit(7),
            "signal" => {
                let result = rustix::process::kill_process(
                    rustix::process::getpid(),
                    rustix::process::Signal::TERM,
                );
                assert!(result.is_ok());
                std::thread::sleep(Duration::from_secs(30));
            }
            "stdout-overflow" => println!("{}", "x".repeat(1024)),
            "stderr-overflow" => eprintln!("{}", "x".repeat(1024)),
            "close-stdin" => std::process::exit(0),
            "file-overflow" => {
                let mut barrier = [0_u8; 1];
                std::io::stdin()
                    .read_exact(&mut barrier)
                    .expect("read file limit barrier");
                assert_eq!(barrier, *b"\n");
                let path = std::env::var_os(OUTPUT_PATH_ENV).expect("output path");
                std::fs::write(path, vec![0_u8; 1024]).expect("file limit terminates write");
            }
            "panic" => {
                std::env::var_os("STAB_BENCH_INTENTIONALLY_MISSING").expect("process helper panic");
            }
            "child-tree" => {
                let child =
                    std::process::Command::new(std::env::current_exe().expect("helper executable"))
                        .args([HELPER_TEST, "--exact", "--ignored", "--nocapture"])
                        .env_clear()
                        .env(HELPER_ENV, "grandchild")
                        .spawn()
                        .expect("spawn grandchild");
                println!("grandchild-pid={}", child.id());
                std::io::stdout().flush().expect("flush grandchild pid");
                drop(child);
                std::thread::sleep(Duration::from_secs(30));
            }
            "affinity" => {
                let expected = std::env::var(EXPECTED_CPU_ENV)
                    .expect("expected CPU")
                    .parse::<usize>()
                    .expect("numeric CPU");
                let set = rustix::thread::sched_getaffinity(None).expect("read child affinity");
                let actual = (0..rustix::thread::CpuSet::MAX_CPU)
                    .filter(|cpu| set.is_set(*cpu))
                    .collect::<Vec<_>>();
                assert_eq!(actual, [expected]);
                println!("affinity-ok");
            }
            "grandchild" => std::thread::sleep(Duration::from_secs(30)),
            other => {
                eprintln!("unknown helper mode {other}");
                std::process::exit(125);
            }
        }
    }
}
