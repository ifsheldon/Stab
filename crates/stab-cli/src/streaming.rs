use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use stab_core::{
    DetectionEventRecord, DetectionObservableOutputMode, SampleFormat,
    result_formats::MeasureRecordWriter,
};

use crate::CliError;

pub(crate) struct FileOutputSink {
    path: PathBuf,
    file: File,
}

impl FileOutputSink {
    pub(crate) fn create(path: &PathBuf) -> Result<Self, CliError> {
        let file = File::create(path).map_err(|source| CliError::WritePath {
            path: path.clone(),
            source,
        })?;
        Ok(Self {
            path: path.clone(),
            file,
        })
    }

    pub(crate) fn write_with(
        &mut self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), CliError> {
        write(&mut self.file).map_err(|source| CliError::WritePath {
            path: self.path.clone(),
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
    pub(crate) fn create(path: Option<&PathBuf>, stdout: &'a mut W) -> Result<Self, CliError> {
        if let Some(path) = path {
            return Ok(Self::File(FileOutputSink::create(path)?));
        }
        Ok(Self::Stdout(stdout))
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

pub(crate) fn write_bits_record<W>(
    bits: &[bool],
    format: SampleFormat,
    output: &mut W,
) -> io::Result<()>
where
    W: Write + ?Sized,
{
    let mut writer = MeasureRecordWriter::new(format);
    writer.write_bits(bits);
    writer.write_end();
    output.write_all(&writer.into_bytes())
}

pub(crate) fn write_detection_record<W>(
    record: &DetectionEventRecord,
    observable_mode: DetectionObservableOutputMode,
    format: SampleFormat,
    output: &mut W,
) -> io::Result<()>
where
    W: Write + ?Sized,
{
    let mut writer = MeasureRecordWriter::new(format);
    if format == SampleFormat::Dets {
        if observable_mode == DetectionObservableOutputMode::Prepend {
            writer.begin_result_type(b'L');
            writer.write_bits(&record.observables);
        }
        writer.begin_result_type(b'D');
        writer.write_bits(&record.detectors);
        if observable_mode == DetectionObservableOutputMode::Append {
            writer.begin_result_type(b'L');
            writer.write_bits(&record.observables);
        }
    } else {
        if observable_mode == DetectionObservableOutputMode::Prepend {
            writer.write_bits(&record.observables);
        }
        writer.write_bits(&record.detectors);
        if observable_mode == DetectionObservableOutputMode::Append {
            writer.write_bits(&record.observables);
        }
    }
    writer.write_end();
    output.write_all(&writer.into_bytes())
}

pub(crate) fn write_observable_record<W>(
    record: &DetectionEventRecord,
    format: SampleFormat,
    output: &mut W,
) -> io::Result<()>
where
    W: Write + ?Sized,
{
    let mut writer = MeasureRecordWriter::new(format);
    if format == SampleFormat::Dets {
        writer.begin_result_type(b'L');
    }
    writer.write_bits(&record.observables);
    writer.write_end();
    output.write_all(&writer.into_bytes())
}

pub(crate) fn detection_record_bits(
    record: &DetectionEventRecord,
    observable_mode: DetectionObservableOutputMode,
) -> Vec<bool> {
    let mut bits = Vec::with_capacity(match observable_mode {
        DetectionObservableOutputMode::DetectorsOnly => record.detectors.len(),
        DetectionObservableOutputMode::Append | DetectionObservableOutputMode::Prepend => {
            record.detectors.len() + record.observables.len()
        }
    });
    if observable_mode == DetectionObservableOutputMode::Prepend {
        bits.extend_from_slice(&record.observables);
    }
    bits.extend_from_slice(&record.detectors);
    if observable_mode == DetectionObservableOutputMode::Append {
        bits.extend_from_slice(&record.observables);
    }
    bits
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
