//! Process-group leaders that are killed and reaped as one unit.
//!
//! A supervised program leads its own process group. The group is always
//! killed while the leader is still unreaped: a zombie leader pins its process
//! id and therefore its process-group id, so the kill can never reach a
//! recycled group that belongs to an unrelated process.

use std::os::fd::OwnedFd;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

/// Converts a child process id to the kernel's signed pid type.
pub fn process_id(pid: u32) -> Result<rustix::process::Pid, std::io::Error> {
    i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("child process id {pid} is outside the supported Linux pid range"),
            )
        })
}

/// A spawned program that leads its own process group.
///
/// Dropping an unreaped leader kills its whole group and reaps the leader.
#[derive(Debug)]
pub struct ProcessGroupLeader {
    child: Child,
    pid: rustix::process::Pid,
    completion: OwnedFd,
    status: Option<ExitStatus>,
}

/// A leader reaped with `wait4`, with the kernel's peak RSS for it.
#[derive(Debug)]
pub struct RusageCompletion {
    pub status: ExitStatus,
    pub peak_rss_bytes: u64,
    pub completed_at: Instant,
}

impl ProcessGroupLeader {
    /// Spawns `command` as the leader of a new process group.
    pub fn spawn(command: &mut Command) -> Result<Self, std::io::Error> {
        use std::os::unix::process::CommandExt as _;

        command.process_group(0);
        let mut child = command.spawn()?;
        match Self::completion_descriptor(&child) {
            Ok((pid, completion)) => Ok(Self {
                child,
                pid,
                completion,
                status: None,
            }),
            Err(source) => {
                // The group exists but cannot be supervised: kill it before
                // the unreaped leader releases its group id.
                if let Ok(pid) = process_id(child.id())
                    && let Err(_ignored) =
                        rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
                {
                    // The kill below still targets the leader itself.
                }
                drop(child.kill());
                drop(child.wait());
                Err(source)
            }
        }
    }

    fn completion_descriptor(
        child: &Child,
    ) -> Result<(rustix::process::Pid, OwnedFd), std::io::Error> {
        let pid = process_id(child.id())?;
        let completion = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::NONBLOCK)?;
        Ok((pid, completion))
    }

    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// The spawned child, for taking its piped standard streams.
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// The leader's status once it has been reaped.
    #[must_use]
    pub const fn status(&self) -> Option<ExitStatus> {
        self.status
    }

    /// Waits up to `timeout` for the leader to exit, without reaping it.
    pub fn wait_for_exit(&self, timeout: Duration) -> Result<bool, std::io::Error> {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};

        if self.status.is_some() {
            return Ok(true);
        }
        let timeout = Timespec::try_from(timeout)
            .map_err(|source| std::io::Error::new(std::io::ErrorKind::InvalidInput, source))?;
        let mut descriptors = [PollFd::new(&self.completion, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => Ok(false),
            Ok(_)
                if descriptors
                    .first()
                    .is_some_and(|descriptor| descriptor.revents().contains(PollFlags::NVAL)) =>
            {
                Err(std::io::Error::other(
                    "child completion descriptor is invalid",
                ))
            }
            Ok(_) => Ok(true),
            Err(source) => Err(source.into()),
        }
    }

    /// Kills the whole group and reaps the leader. A leader that already
    /// exited stays a zombie until this reap, so the kill still targets its
    /// own group.
    pub fn kill_group_and_reap(&mut self) -> Result<ExitStatus, std::io::Error> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        self.kill_unreaped_group()?;
        let status = self.child.wait()?;
        self.status = Some(status);
        Ok(status)
    }

    /// Like [`Self::kill_group_and_reap`], reaping with `wait4` to observe the
    /// leader's peak RSS. Call it only after [`Self::wait_for_exit`] returned
    /// true, so the reap cannot block.
    pub fn kill_group_and_reap_with_rusage(&mut self) -> Result<RusageCompletion, std::io::Error> {
        if self.status.is_some() {
            return Err(std::io::Error::other(
                "process-group leader was already reaped",
            ));
        }
        self.kill_unreaped_group()?;
        let completed = wait_with_rusage(self.pid)?;
        self.status = Some(completed.status);
        Ok(completed)
    }

    fn kill_unreaped_group(&self) -> Result<(), std::io::Error> {
        match rustix::process::kill_process_group(self.pid, rustix::process::Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
            Err(source) => Err(source.into()),
        }
    }
}

impl Drop for ProcessGroupLeader {
    fn drop(&mut self) {
        if self.status.is_none() {
            if self.kill_unreaped_group().is_err() {
                drop(self.child.kill());
            }
            drop(self.child.wait());
        }
    }
}

/// Reaps `pid` with `wait4` and reports its peak RSS. The caller must know the
/// process has exited or accept that the call blocks until it does.
#[allow(
    unsafe_code,
    reason = "Linux wait4 is the only per-child peak-RSS source that survives fast process exit"
)]
pub(crate) fn wait_with_rusage(
    pid: rustix::process::Pid,
) -> Result<RusageCompletion, std::io::Error> {
    use std::mem::MaybeUninit;
    use std::os::unix::process::ExitStatusExt as _;

    let raw_pid = pid.as_raw_nonzero().get();
    let mut status = 0;
    let mut usage = MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: wait4 receives a known child PID and valid writable pointers. A positive
    // return initializes both outputs; negative returns errno.
    let waited = unsafe { libc::wait4(raw_pid, &mut status, 0, usage.as_mut_ptr()) };
    if waited < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let completed_at = Instant::now();
    // SAFETY: a positive wait4 return initialized the rusage value.
    let usage = unsafe { usage.assume_init() };
    let peak_kib = u64::try_from(usage.ru_maxrss)
        .map_err(|_| std::io::Error::other("wait4 returned negative peak RSS"))?;
    let peak_rss_bytes = peak_kib
        .checked_mul(1024)
        .ok_or_else(|| std::io::Error::other("wait4 peak RSS overflows bytes"))?;
    Ok(RusageCompletion {
        status: ExitStatus::from_raw(status),
        peak_rss_bytes,
        completed_at,
    })
}
