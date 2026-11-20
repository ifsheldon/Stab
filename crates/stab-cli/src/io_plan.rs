use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use same_file::Handle;

use crate::CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileRole {
    Input,
    Circuit,
    Dem,
    Sweep,
    ReplayErrorInput,
    Output,
    ObservableOutput,
    ErrorOutput,
}

impl FileRole {
    pub(crate) const fn flag(self) -> &'static str {
        match self {
            Self::Input => "--in",
            Self::Circuit => "--circuit",
            Self::Dem => "--dem",
            Self::Sweep => "--sweep",
            Self::ReplayErrorInput => "--replay_err_in",
            Self::Output => "--out",
            Self::ObservableOutput => "--obs_out",
            Self::ErrorOutput => "--err_out",
        }
    }
}

/// Identity of a standard stream, retained only when it is a regular file.
///
/// Pipes, terminals, and character devices such as `/dev/null` cannot lose
/// data through aliasing, so they are never compared.
#[derive(Debug, Default)]
pub(crate) struct StandardStreams {
    stdin: Option<Handle>,
    stdout: Option<Handle>,
}

impl StandardStreams {
    /// Streams that are not process standard streams, such as in-memory buffers.
    pub(crate) fn detached() -> Self {
        Self::default()
    }

    /// Captures the identities of this process's standard input and output.
    pub(crate) fn capture() -> Self {
        Self {
            stdin: regular_file_handle(Handle::stdin()),
            stdout: regular_file_handle(Handle::stdout()),
        }
    }
}

fn regular_file_handle(handle: std::io::Result<Handle>) -> Option<Handle> {
    let handle = handle.ok()?;
    let is_file = handle.as_file().metadata().ok()?.file_type().is_file();
    is_file.then_some(handle)
}

/// Which standard streams a command will actually read or write.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StreamUse {
    pub(crate) reads_stdin: bool,
    pub(crate) writes_stdout: bool,
}

const STDIN_ROLE: &str = "stdin";
const STDOUT_ROLE: &str = "stdout";

#[derive(Debug)]
pub(crate) struct InputFile {
    role: FileRole,
    path: PathBuf,
    identity: Handle,
}

impl InputFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn len(&self) -> Result<u64, CliError> {
        self.identity
            .as_file()
            .metadata()
            .map(|metadata| metadata.len())
            .map_err(|source| CliError::ReadPath {
                path: self.path.clone(),
                source,
            })
    }

    pub(crate) fn regular_file_len(&self) -> Result<Option<u64>, CliError> {
        let metadata = self
            .identity
            .as_file()
            .metadata()
            .map_err(|source| CliError::ReadPath {
                path: self.path.clone(),
                source,
            })?;
        Ok(metadata.file_type().is_file().then_some(metadata.len()))
    }

    /// The opened file itself, whose reads fill uninitialized buffer capacity directly.
    pub(crate) fn file_mut(&mut self) -> &mut File {
        self.identity.as_file_mut()
    }

    /// Positions the next read at absolute byte `offset`; a non-seekable input fails here.
    pub(crate) fn seek_to(&mut self, offset: u64) -> Result<(), CliError> {
        self.seek(SeekFrom::Start(offset))
            .map(|_| ())
            .map_err(|source| CliError::ReadPath {
                path: self.path.clone(),
                source,
            })
    }
}

impl Read for InputFile {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.identity.as_file_mut().read(buffer)
    }
}

impl Seek for InputFile {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.identity.as_file_mut().seek(position)
    }
}

/// Bytes buffered in front of every command output stream, file or standard output.
pub(crate) const OUTPUT_BUFFER_BYTES: usize = 64 * 1024;

/// Wraps a command output stream in the shared output buffer.
///
/// Dropping a [`BufWriter`] flushes but discards any error, so every successful command path
/// must flush explicitly and surface the result; a drop-time flush only happens on paths that are
/// already reporting a failure.
pub(crate) fn buffered_output<W>(output: W) -> BufWriter<W>
where
    W: Write,
{
    BufWriter::with_capacity(OUTPUT_BUFFER_BYTES, output)
}

/// An activated output file whose writes pass through [`buffered_output`].
#[derive(Debug)]
pub(crate) struct OutputFile {
    role: FileRole,
    path: PathBuf,
    writer: BufWriter<OutputTarget>,
}

impl OutputFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Builds an output file over an injected writer so tests can observe write and flush calls.
    #[cfg(test)]
    pub(crate) fn probe(path: &Path, writer: Box<dyn Write>) -> Self {
        Self {
            role: FileRole::Output,
            path: path.to_path_buf(),
            writer: buffered_output(OutputTarget::Probe(writer)),
        }
    }
}

impl Write for OutputFile {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buffer)
    }

    fn write_all(&mut self, buffer: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// The destination behind an output file's buffer.
enum OutputTarget {
    /// The retained handle opened and alias-checked during preflight.
    Retained(Handle),
    #[cfg(test)]
    Probe(Box<dyn Write>),
}

impl std::fmt::Debug for OutputTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retained(handle) => formatter.debug_tuple("Retained").field(handle).finish(),
            #[cfg(test)]
            Self::Probe(_) => formatter.write_str("Probe"),
        }
    }
}

impl Write for OutputTarget {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Retained(handle) => handle.as_file_mut().write(buffer),
            #[cfg(test)]
            Self::Probe(writer) => writer.write(buffer),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Retained(handle) => handle.as_file_mut().flush(),
            #[cfg(test)]
            Self::Probe(writer) => writer.flush(),
        }
    }
}

#[derive(Debug)]
struct PendingOutput {
    role: FileRole,
    path: PathBuf,
    identity: Option<Handle>,
    is_regular_file: bool,
}

#[derive(Debug)]
pub(crate) struct PendingIo {
    inputs: Vec<InputFile>,
    outputs: Vec<PendingOutput>,
}

impl PendingIo {
    pub(crate) fn reject_aliases_without_opening<'input, 'output>(
        inputs: impl IntoIterator<Item = (FileRole, Option<&'input Path>)>,
        outputs: impl IntoIterator<Item = (FileRole, Option<&'output Path>)>,
    ) -> Result<(), CliError> {
        let inputs = inputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .collect::<Vec<_>>();
        let outputs = outputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .collect::<Vec<_>>();
        for (input_role, input_path) in &inputs {
            for (output_role, output_path) in &outputs {
                if paths_alias_without_creation(input_path, output_path) {
                    return Err(CliError::ConflictingFileRoles {
                        first: input_role.flag(),
                        second: output_role.flag(),
                    });
                }
            }
        }
        for (index, (role, path)) in outputs.iter().enumerate() {
            for (other_role, other_path) in outputs.iter().skip(index + 1) {
                if paths_alias_without_creation(path, other_path) {
                    return Err(CliError::ConflictingFileRoles {
                        first: role.flag(),
                        second: other_role.flag(),
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn preflight<'input, 'output>(
        inputs: impl IntoIterator<Item = (FileRole, Option<&'input Path>)>,
        outputs: impl IntoIterator<Item = (FileRole, Option<&'output Path>)>,
    ) -> Result<Self, CliError> {
        Self::preflight_inputs(inputs)?.with_outputs(outputs)
    }

    /// Plans the named outputs of a command that reads no named input.
    pub(crate) fn preflight_outputs<'output>(
        outputs: impl IntoIterator<Item = (FileRole, Option<&'output Path>)>,
    ) -> Result<Self, CliError> {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
        .with_outputs(outputs)
    }

    pub(crate) fn preflight_inputs<'input>(
        inputs: impl IntoIterator<Item = (FileRole, Option<&'input Path>)>,
    ) -> Result<Self, CliError> {
        let inputs = inputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .map(|(role, path)| open_input(role, path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inputs,
            outputs: Vec::new(),
        })
    }

    pub(crate) fn with_outputs<'output>(
        mut self,
        outputs: impl IntoIterator<Item = (FileRole, Option<&'output Path>)>,
    ) -> Result<Self, CliError> {
        let outputs = outputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .map(|(role, path)| open_output(role, path))
            .collect::<Result<Vec<_>, _>>()?;

        reject_input_output_aliases(&self.inputs, &outputs)?;
        reject_output_aliases(&outputs)?;
        self.outputs = outputs;
        Ok(self)
    }

    /// Rejects a standard stream that refers to the same regular file as a
    /// command input or output before any output is truncated.
    pub(crate) fn reject_standard_stream_aliases(
        &self,
        streams: &StandardStreams,
        used: StreamUse,
    ) -> Result<(), CliError> {
        let stdin = streams.stdin.as_ref().filter(|_| used.reads_stdin);
        let stdout = streams.stdout.as_ref().filter(|_| used.writes_stdout);
        if let Some(stdin) = stdin {
            if let Some(output) = self
                .outputs
                .iter()
                .find(|output| output.identity.as_ref() == Some(stdin))
            {
                return Err(CliError::ConflictingFileRoles {
                    first: STDIN_ROLE,
                    second: output.role.flag(),
                });
            }
            if stdout == Some(stdin) {
                return Err(CliError::ConflictingFileRoles {
                    first: STDIN_ROLE,
                    second: STDOUT_ROLE,
                });
            }
        }
        if let Some(stdout) = stdout {
            if let Some(input) = self.inputs.iter().find(|input| input.identity == *stdout) {
                return Err(CliError::ConflictingFileRoles {
                    first: input.role.flag(),
                    second: STDOUT_ROLE,
                });
            }
            if let Some(output) = self
                .outputs
                .iter()
                .find(|output| output.identity.as_ref() == Some(stdout))
            {
                return Err(CliError::ConflictingFileRoles {
                    first: output.role.flag(),
                    second: STDOUT_ROLE,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn take_input(&mut self, role: FileRole) -> Option<InputFile> {
        let index = self.inputs.iter().position(|input| input.role == role)?;
        Some(self.inputs.remove(index))
    }

    pub(crate) fn input_mut(&mut self, role: FileRole) -> Option<&mut InputFile> {
        self.inputs.iter_mut().find(|input| input.role == role)
    }

    pub(crate) fn activate(mut self) -> Result<ActiveOutputs, CliError> {
        for output in &mut self.outputs {
            let identity = output.identity.as_mut().ok_or(CliError::IoPlanInvariant {
                message: "pending output lost its retained identity before activation",
            })?;
            if output.is_regular_file {
                identity
                    .as_file_mut()
                    .set_len(0)
                    .map_err(|source| CliError::WritePath {
                        path: output.path.clone(),
                        source,
                    })?;
            }
        }

        let outputs = self
            .outputs
            .drain(..)
            .map(|mut output| -> Result<OutputFile, CliError> {
                let identity = output.identity.take().ok_or(CliError::IoPlanInvariant {
                    message: "activated output lost its retained identity",
                })?;
                Ok(OutputFile {
                    role: output.role,
                    path: output.path.clone(),
                    writer: buffered_output(OutputTarget::Retained(identity)),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveOutputs { outputs })
    }
}

/// Whether two role paths name the same file that one role could truncate under another.
///
/// This pre-open check only spares an output from being created or opened when it would be
/// refused anyway. A shared target that already exists and is not a regular file, such as
/// `/dev/null` or a FIFO, cannot be truncated through either role, so it is not an alias; the
/// authoritative comparison happens on the opened handles in [`PendingIo::with_outputs`], which
/// takes the file type from the handle itself, so a path swapped to a regular file after this
/// lookup is still refused before anything is truncated.
fn paths_alias_without_creation(left: &Path, right: &Path) -> bool {
    let aliases = left == right
        || lexical_absolute(left)
            .is_some_and(|left| lexical_absolute(right).is_some_and(|right| left == right))
        || (left.exists()
            && right.exists()
            && same_file::is_same_file(left, right).unwrap_or(false));
    aliases && !names_existing_non_regular_file(left)
}

/// Whether `path` resolves to an existing file that is not a regular file; a path that does not
/// exist yet would be created as a regular file.
fn names_existing_non_regular_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| !metadata.file_type().is_file())
}

fn lexical_absolute(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    Some(normalized)
}

#[derive(Debug)]
pub(crate) struct ActiveOutputs {
    outputs: Vec<OutputFile>,
}

impl ActiveOutputs {
    pub(crate) fn take(&mut self, role: FileRole) -> Option<OutputFile> {
        let index = self.outputs.iter().position(|output| output.role == role)?;
        Some(self.outputs.remove(index))
    }
}

fn open_input(role: FileRole, path: &Path) -> Result<InputFile, CliError> {
    let file = File::open(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    let identity = Handle::from_file(file).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(InputFile {
        role,
        path: path.to_path_buf(),
        identity,
    })
}

fn open_output(role: FileRole, path: &Path) -> Result<PendingOutput, CliError> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|source| CliError::WritePath {
            path: path.to_path_buf(),
            source,
        })?;
    let is_regular_file = file
        .metadata()
        .map_err(|source| CliError::WritePath {
            path: path.to_path_buf(),
            source,
        })?
        .file_type()
        .is_file();
    let identity = Handle::from_file(file).map_err(|source| CliError::WritePath {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(PendingOutput {
        role,
        path: path.to_path_buf(),
        identity: Some(identity),
        is_regular_file,
    })
}

/// Refuses an output whose opened handle is the same regular file as an input. Two handles to
/// one file share its type, so the output handle's type decides for both; a shared device or FIFO
/// cannot be truncated and is allowed.
fn reject_input_output_aliases(
    inputs: &[InputFile],
    outputs: &[PendingOutput],
) -> Result<(), CliError> {
    for input in inputs {
        for output in outputs.iter().filter(|output| output.is_regular_file) {
            if output
                .identity
                .as_ref()
                .is_some_and(|identity| input.identity == *identity)
            {
                return Err(CliError::ConflictingFileRoles {
                    first: input.role.flag(),
                    second: output.role.flag(),
                });
            }
        }
    }
    Ok(())
}

/// Refuses two outputs whose opened handles are the same regular file, so activating one cannot
/// truncate what the other wrote; two roles writing one device or FIFO, such as `/dev/null`, are
/// allowed as Stim allows them.
fn reject_output_aliases(outputs: &[PendingOutput]) -> Result<(), CliError> {
    for (index, output) in outputs.iter().enumerate() {
        if !output.is_regular_file {
            continue;
        }
        for other in outputs.iter().skip(index + 1) {
            if output.identity == other.identity {
                return Err(CliError::ConflictingFileRoles {
                    first: output.role.flag(),
                    second: other.role.flag(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use tempfile::tempdir;

    use super::{FileRole, PendingIo};
    use crate::CliError;

    #[test]
    fn delayed_output_open_rechecks_retained_input_identity() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.dem");
        let output_path = dir.path().join("output.01");
        std::fs::write(&input_path, b"sentinel\n").expect("write input");
        let io = PendingIo::preflight_inputs([(FileRole::Input, Some(input_path.as_path()))])
            .expect("retain input identity");

        std::fs::hard_link(&input_path, &output_path)
            .expect("replace delayed output path with input hardlink");
        let error = io
            .with_outputs([(FileRole::Output, Some(output_path.as_path()))])
            .expect_err("delayed identity comparison must reject the new alias");

        assert_matches!(
            error,
            CliError::ConflictingFileRoles {
                first: "--in",
                second: "--out"
            }
        );
        assert_eq!(
            std::fs::read(input_path).expect("read preserved input"),
            b"sentinel\n"
        );
    }
}
