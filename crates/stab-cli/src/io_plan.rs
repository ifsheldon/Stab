use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

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

    pub(crate) fn rewind(&mut self) -> Result<(), CliError> {
        self.seek(SeekFrom::Start(0))
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

#[derive(Debug)]
pub(crate) struct OutputFile {
    role: FileRole,
    path: PathBuf,
    identity: Handle,
}

impl OutputFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Write for OutputFile {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.identity.as_file_mut().write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.identity.as_file_mut().flush()
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
    pub(crate) fn preflight<'input, 'output>(
        inputs: impl IntoIterator<Item = (FileRole, Option<&'input Path>)>,
        outputs: impl IntoIterator<Item = (FileRole, Option<&'output Path>)>,
    ) -> Result<Self, CliError> {
        let inputs = inputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .map(|(role, path)| open_input(role, path))
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = outputs
            .into_iter()
            .filter_map(|(role, path)| path.map(|path| (role, path)))
            .map(|(role, path)| open_output(role, path))
            .collect::<Result<Vec<_>, _>>()?;

        reject_input_output_aliases(&inputs, &outputs)?;
        reject_output_aliases(&outputs)?;
        Ok(Self { inputs, outputs })
    }

    pub(crate) fn take_input(&mut self, role: FileRole) -> Option<InputFile> {
        let index = self.inputs.iter().position(|input| input.role == role)?;
        Some(self.inputs.remove(index))
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
                    identity,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveOutputs { outputs })
    }
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

fn reject_input_output_aliases(
    inputs: &[InputFile],
    outputs: &[PendingOutput],
) -> Result<(), CliError> {
    for input in inputs {
        for output in outputs {
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

fn reject_output_aliases(outputs: &[PendingOutput]) -> Result<(), CliError> {
    for (index, output) in outputs.iter().enumerate() {
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
