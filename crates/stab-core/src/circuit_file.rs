use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::Path,
};

use crate::{Circuit, CircuitError, CircuitResult};

const MAX_CIRCUIT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CIRCUIT_FILE_BYTES_USIZE: usize = 64 * 1024 * 1024;
const CIRCUIT_FILE_READ_LIMIT: u64 = MAX_CIRCUIT_FILE_BYTES + 1;

/// Reads a bounded `.stim` circuit file from a filesystem path.
///
/// Files larger than 64 MiB are rejected before byte-oriented model parsing.
pub fn read_stim_circuit_file(path: impl AsRef<Path>) -> CircuitResult<Circuit> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| CircuitError::circuit_io("read", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| CircuitError::circuit_io("read", error))?;
    if metadata.len() > MAX_CIRCUIT_FILE_BYTES {
        return Err(circuit_file_size_error(metadata.len()));
    }

    let mut bytes = Vec::new();
    file.take(CIRCUIT_FILE_READ_LIMIT)
        .read_to_end(&mut bytes)
        .map_err(|error| CircuitError::circuit_io("read", error))?;
    if bytes.len() > MAX_CIRCUIT_FILE_BYTES_USIZE {
        return Err(circuit_file_size_error(CIRCUIT_FILE_READ_LIMIT));
    }

    Circuit::from_stim_bytes(&bytes).map_err(Into::into)
}

/// Writes canonical `.stim` circuit text to a filesystem path.
pub fn write_stim_circuit_file(circuit: &Circuit, path: impl AsRef<Path>) -> CircuitResult<()> {
    let file =
        File::create(path.as_ref()).map_err(|error| CircuitError::circuit_io("write", error))?;
    let mut writer = BufWriter::new(file);
    circuit
        .write_stim_io(&mut writer)
        .and_then(|()| writer.flush())
        .map_err(|error| CircuitError::circuit_io("write", error))
}

fn circuit_file_size_error(size: u64) -> CircuitError {
    CircuitError::invalid_domain_value(
        "circuit file size",
        format!("{size} bytes exceeds {MAX_CIRCUIT_FILE_BYTES} byte limit"),
    )
}
