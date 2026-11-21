//! Stoppable bounded pipe I/O for supervised process groups.
//!
//! Each reader and writer runs on its own thread over a nonblocking pipe end
//! and waits with `poll`, so a supervisor can always stop it. Stopping never
//! discards data that is already readable: a stopped reader keeps draining
//! until the pipe reports end of file or would block, and only then gives a
//! descendant that still holds the write end a bounded grace to close it.

use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const POLL_SLICE: Duration = Duration::from_millis(10);
const READ_CHUNK_BYTES: usize = 16 << 10;
/// Bytes a stopped reader may still drain after its grace expired before it
/// declares the capture truncated. A pipe holds at most 1 MiB by default
/// (`/proc/sys/fs/pipe-max-size`), so a writer that is merely flushing its
/// buffer stays below this bound while an unbounded writer cannot hold the
/// reader forever.
const POST_GRACE_DRAIN_BYTES: u64 = 4 << 20;

/// What a reader keeps from its stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPolicy {
    /// Keep at most `maximum_bytes`; any further byte marks the stream as
    /// exceeded while the reader keeps draining and counting.
    Capture { maximum_bytes: usize },
    /// Drain and count the stream without keeping it.
    Discard,
}

impl OutputPolicy {
    pub(crate) const fn maximum_bytes(self) -> Option<usize> {
        match self {
            Self::Capture { maximum_bytes } => Some(maximum_bytes),
            Self::Discard => None,
        }
    }
}

impl From<usize> for OutputPolicy {
    fn from(maximum_bytes: usize) -> Self {
        Self::Capture { maximum_bytes }
    }
}

/// How a pipe end behaves once its supervisor asks it to stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainPolicy {
    /// Ignore stop requests and run until every writer closes the pipe.
    UntilEof,
    /// After a stop request, drain everything readable, then wait at most
    /// `grace` for end of file. A pipe that is still open afterwards is held by
    /// a process outside the killed group and is reported as
    /// [`StreamEnd::HeldOpen`].
    StopAfterGrace(Duration),
}

/// Why a stream ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamEnd {
    /// Every writer closed the pipe after all data was read.
    Eof,
    /// The stopped stream had no readable data left, but some process still
    /// held the other end open when the grace expired.
    HeldOpen,
    /// Readable data was still arriving when the reader gave up, so the
    /// capture is incomplete.
    Truncated,
}

impl StreamEnd {
    /// Whether the stream ended because every writer closed it.
    #[must_use]
    pub const fn is_eof(self) -> bool {
        matches!(self, Self::Eof)
    }
}

/// The bounded result of one drained output stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedStream {
    /// The retained prefix of the stream; empty for [`OutputPolicy::Discard`].
    pub bytes: Vec<u8>,
    /// Every byte read from the stream, retained or not.
    pub total_bytes: u64,
    /// Whether the stream crossed its capture limit.
    pub exceeded: bool,
    pub end: StreamEnd,
}

/// A thread draining one child output pipe.
#[derive(Debug)]
pub struct OutputReader {
    handle: JoinHandle<Result<CapturedStream, std::io::Error>>,
    stop: Arc<AtomicBool>,
    exceeded: Arc<AtomicBool>,
}

impl OutputReader {
    /// Starts draining `pipe`, which becomes nonblocking. Only the parent's
    /// end of the pipe changes: the child's end is a separate open file
    /// description.
    pub fn spawn<R>(
        pipe: R,
        policy: OutputPolicy,
        drain: DrainPolicy,
    ) -> Result<Self, std::io::Error>
    where
        R: Read + AsFd + Send + 'static,
    {
        set_nonblocking(&pipe)?;
        let stop = Arc::new(AtomicBool::new(false));
        let exceeded = Arc::new(AtomicBool::new(false));
        let reader_stop = Arc::clone(&stop);
        let reader_exceeded = Arc::clone(&exceeded);
        let handle = std::thread::Builder::new()
            .name("stab-process-reader".to_string())
            .spawn(move || read_stream(pipe, policy, drain, &reader_stop, &reader_exceeded))?;
        Ok(Self {
            handle,
            stop,
            exceeded,
        })
    }

    /// Asks the reader to finish under its [`DrainPolicy`].
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    /// Whether the stream has crossed its capture limit so far.
    #[must_use]
    pub fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Waits for the reader. It returns within its drain grace once stopped,
    /// unless its policy is [`DrainPolicy::UntilEof`].
    pub fn join(self) -> Result<CapturedStream, std::io::Error> {
        self.handle
            .join()
            .unwrap_or_else(|_| Err(std::io::Error::other("output reader thread panicked")))
    }
}

fn read_stream<R: Read + AsFd>(
    mut pipe: R,
    policy: OutputPolicy,
    drain: DrainPolicy,
    stop: &AtomicBool,
    exceeded: &AtomicBool,
) -> Result<CapturedStream, std::io::Error> {
    let mut captured = CapturedStream {
        bytes: Vec::new(),
        total_bytes: 0,
        exceeded: false,
        end: StreamEnd::Eof,
    };
    let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
    let mut stop_deadline = StopDeadline::new(drain);
    let mut drained_after_grace = 0_u64;
    loop {
        stop_deadline.observe(stop);
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(captured),
            Ok(count) => {
                let chunk = buffer
                    .get(..count)
                    .ok_or_else(|| std::io::Error::other("pipe read exceeded its buffer"))?;
                record_chunk(&mut captured, chunk, policy, exceeded)?;
                if stop_deadline.expired() {
                    drained_after_grace = drained_after_grace.saturating_add(byte_count(count)?);
                    if drained_after_grace > POST_GRACE_DRAIN_BYTES {
                        captured.end = StreamEnd::Truncated;
                        return Ok(captured);
                    }
                }
            }
            Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                if stop_deadline.expired() {
                    captured.end = StreamEnd::HeldOpen;
                    return Ok(captured);
                }
                wait_ready(&pipe, rustix::event::PollFlags::IN, stop_deadline.wait())?;
            }
            Err(source) if source.kind() == std::io::ErrorKind::Interrupted => {}
            Err(source) => return Err(source),
        }
    }
}

fn record_chunk(
    captured: &mut CapturedStream,
    chunk: &[u8],
    policy: OutputPolicy,
    exceeded: &AtomicBool,
) -> Result<(), std::io::Error> {
    captured.total_bytes = captured
        .total_bytes
        .checked_add(byte_count(chunk.len())?)
        .ok_or_else(|| std::io::Error::other("pipe byte count overflow"))?;
    if let OutputPolicy::Capture { maximum_bytes } = policy {
        let remaining = maximum_bytes.saturating_sub(captured.bytes.len());
        let kept = chunk
            .get(..remaining.min(chunk.len()))
            .ok_or_else(|| std::io::Error::other("pipe keep exceeded its buffer"))?;
        captured.bytes.extend_from_slice(kept);
        if kept.len() < chunk.len() {
            captured.exceeded = true;
            exceeded.store(true, Ordering::Release);
        }
    }
    Ok(())
}

fn byte_count(count: usize) -> Result<u64, std::io::Error> {
    u64::try_from(count).map_err(|_| std::io::Error::other("pipe byte count does not fit in u64"))
}

/// How a pipe writer finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteEnd {
    /// Every byte was written and the pipe was closed.
    Complete,
    /// The stopped writer could not finish because the reading end was still
    /// open but never drained when the grace expired.
    HeldOpen,
}

/// A thread writing one bounded buffer to a child's stdin.
#[derive(Debug)]
pub struct InputWriter {
    handle: JoinHandle<Result<WriteEnd, std::io::Error>>,
    stop: Arc<AtomicBool>,
}

impl InputWriter {
    /// Starts writing `bytes` to `pipe`, which becomes nonblocking, and closes
    /// the pipe once every byte is written.
    pub fn spawn<W>(pipe: W, bytes: Vec<u8>, drain: DrainPolicy) -> Result<Self, std::io::Error>
    where
        W: Write + AsFd + Send + 'static,
    {
        set_nonblocking(&pipe)?;
        let stop = Arc::new(AtomicBool::new(false));
        let writer_stop = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("stab-process-writer".to_string())
            .spawn(move || write_stream(pipe, &bytes, drain, &writer_stop))?;
        Ok(Self { handle, stop })
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub fn join(self) -> Result<WriteEnd, std::io::Error> {
        self.handle
            .join()
            .unwrap_or_else(|_| Err(std::io::Error::other("stdin writer thread panicked")))
    }
}

fn write_stream<W: Write + AsFd>(
    mut pipe: W,
    bytes: &[u8],
    drain: DrainPolicy,
    stop: &AtomicBool,
) -> Result<WriteEnd, std::io::Error> {
    let mut written = 0;
    let mut stop_deadline = StopDeadline::new(drain);
    while let Some(remaining) = bytes
        .get(written..)
        .filter(|remaining| !remaining.is_empty())
    {
        stop_deadline.observe(stop);
        match pipe.write(remaining) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(count) => written = written.saturating_add(count),
            Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                if stop_deadline.expired() {
                    return Ok(WriteEnd::HeldOpen);
                }
                wait_ready(&pipe, rustix::event::PollFlags::OUT, stop_deadline.wait())?;
            }
            Err(source) if source.kind() == std::io::ErrorKind::Interrupted => {}
            Err(source) => return Err(source),
        }
    }
    Ok(WriteEnd::Complete)
}

/// The grace deadline a stopped pipe end runs against.
struct StopDeadline {
    grace: Option<Duration>,
    deadline: Option<Instant>,
}

impl StopDeadline {
    const fn new(drain: DrainPolicy) -> Self {
        Self {
            grace: match drain {
                DrainPolicy::UntilEof => None,
                DrainPolicy::StopAfterGrace(grace) => Some(grace),
            },
            deadline: None,
        }
    }

    fn observe(&mut self, stop: &AtomicBool) {
        if self.deadline.is_none()
            && let Some(grace) = self.grace
            && stop.load(Ordering::Acquire)
        {
            let now = Instant::now();
            self.deadline = Some(now.checked_add(grace).unwrap_or(now));
        }
    }

    fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn wait(&self) -> Duration {
        self.deadline.map_or(POLL_SLICE, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .min(POLL_SLICE)
        })
    }
}

fn wait_ready(
    pipe: &impl AsFd,
    events: rustix::event::PollFlags,
    timeout: Duration,
) -> Result<(), std::io::Error> {
    use rustix::event::{PollFd, Timespec, poll};

    let timeout = Timespec::try_from(timeout)
        .map_err(|source| std::io::Error::new(std::io::ErrorKind::InvalidInput, source))?;
    let mut descriptors = [PollFd::new(pipe, events)];
    match poll(&mut descriptors, Some(&timeout)) {
        Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
        Err(source) => Err(source.into()),
    }
}

fn set_nonblocking(pipe: &impl AsFd) -> Result<(), std::io::Error> {
    let flags = rustix::fs::fcntl_getfl(pipe)?;
    rustix::fs::fcntl_setfl(pipe, flags | rustix::fs::OFlags::NONBLOCK).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipe_pair() -> (std::fs::File, std::fs::File) {
        let (reader, writer) = rustix::pipe::pipe().expect("pipe");
        (std::fs::File::from(reader), std::fs::File::from(writer))
    }

    #[test]
    fn stopped_reader_drains_buffered_data_after_its_grace_has_passed() {
        let (reader, mut writer) = pipe_pair();
        writer.write_all(&[7_u8; 48 << 10]).expect("fill pipe");
        drop(writer);
        // The reader starts only after its stop grace already expired, which
        // is the late-scheduled reader that used to return an empty capture.
        let reader = OutputReader::spawn(
            reader,
            OutputPolicy::Capture {
                maximum_bytes: 64 << 10,
            },
            DrainPolicy::StopAfterGrace(Duration::ZERO),
        )
        .expect("reader");
        reader.stop();
        let captured = reader.join().expect("joined reader");
        assert_eq!(captured.bytes.len(), 48 << 10);
        assert_eq!(captured.end, StreamEnd::Eof);
    }

    #[test]
    fn stopped_reader_reports_a_held_pipe_instead_of_blocking() {
        let (reader, mut writer) = pipe_pair();
        writer.write_all(b"partial").expect("write prefix");
        let reader = OutputReader::spawn(
            reader,
            OutputPolicy::Capture {
                maximum_bytes: 1024,
            },
            DrainPolicy::StopAfterGrace(Duration::from_millis(20)),
        )
        .expect("reader");
        reader.stop();
        let started = Instant::now();
        let captured = reader.join().expect("joined reader");
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(captured.bytes, b"partial");
        assert_eq!(captured.end, StreamEnd::HeldOpen);
        drop(writer);
    }

    #[test]
    fn stopped_reader_marks_an_endless_stream_as_truncated() {
        // `/dev/zero` is always readable and never ends, like a writer outside
        // the killed group that keeps flooding the pipe.
        let endless = std::fs::File::open("/dev/zero").expect("endless stream");
        let reader = OutputReader::spawn(
            endless,
            OutputPolicy::Discard,
            DrainPolicy::StopAfterGrace(Duration::ZERO),
        )
        .expect("reader");
        reader.stop();
        let captured = reader.join().expect("joined reader");
        assert_eq!(captured.end, StreamEnd::Truncated);
        assert!(captured.total_bytes > POST_GRACE_DRAIN_BYTES);
    }

    #[test]
    fn stopped_writer_reports_an_undrained_reader() {
        let (reader, writer) = pipe_pair();
        let writer = InputWriter::spawn(
            writer,
            vec![0_u8; 4 << 20],
            DrainPolicy::StopAfterGrace(Duration::from_millis(20)),
        )
        .expect("writer");
        std::thread::sleep(Duration::from_millis(20));
        writer.stop();
        assert_eq!(writer.join().expect("joined writer"), WriteEnd::HeldOpen);
        drop(reader);
    }
}
