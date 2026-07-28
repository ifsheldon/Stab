use std::io::{self, Write};

use crate::{CliError, io_plan::OutputFile};

pub(crate) struct FileOutputSink {
    file: OutputFile,
}

impl FileOutputSink {
    pub(crate) fn from_output(file: OutputFile) -> Self {
        Self { file }
    }

    pub(crate) fn write_with(
        &mut self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), CliError> {
        write(&mut self.file).map_err(|source| CliError::WritePath {
            path: self.file.path().to_path_buf(),
            source,
        })
    }
}

pub(crate) enum OutputSink<'a, W>
where
    W: Write,
{
    Stdout(&'a mut W),
    File(FileOutputSink),
}

impl<'a, W> OutputSink<'a, W>
where
    W: Write,
{
    pub(crate) fn from_output(output: Option<OutputFile>, stdout: &'a mut W) -> Self {
        match output {
            Some(output) => Self::File(FileOutputSink::from_output(output)),
            None => Self::Stdout(stdout),
        }
    }

    pub(crate) fn write_with(
        &mut self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), CliError> {
        match self {
            Self::Stdout(stdout) => write(*stdout).map_err(CliError::WriteOutput),
            Self::File(sink) => sink.write_with(write),
        }
    }
}

pub(crate) fn write_ptb64_group<W>(records: &[Vec<bool>], output: &mut W) -> io::Result<()>
where
    W: Write + ?Sized,
{
    let bits_per_record = records.first().map_or(0, Vec::len);
    let mut words = vec![0u64; bits_per_record];
    for (shot_index, record) in records.iter().enumerate() {
        if record.len() != bits_per_record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "internal sampler emitted non-uniform ptb64 records",
            ));
        }
        for (bit_index, bit) in record.iter().enumerate() {
            if *bit {
                let word = words.get_mut(bit_index).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "internal sampler emitted ptb64 bit outside the record width",
                    )
                })?;
                *word |= 1u64 << shot_index;
            }
        }
    }
    for word in words {
        output.write_all(&word.to_le_bytes())?;
    }
    Ok(())
}
