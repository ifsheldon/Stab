//! Conservative bounds on what one [`RecordStreamReader`] retains, computed before it exists so a
//! caller can admit the storage against its memory budget before any of it is allocated.

use super::INITIAL_BUFFER_BYTES;
#[cfg(doc)]
use super::RecordStreamReader;
use crate::{PackedShotBatch, RecordFormat};

/// Records in one PTB64 group.
const PTB64_GROUP_RECORDS: usize = u64::BITS as usize;

/// Upper bound on the bytes one [`RecordStreamReader`] of a format and width retains.
///
/// The bound covers the reusable record batch for the largest record count a caller passes to
/// [`RecordStreamReader::read_batch`], the PTB64 bit-plane and transposed-row group, and for the
/// packed formats (`b8`, `r8`, and PTB64) the transport buffer at the size its growth policy
/// reaches while one worst-case record frame spans it, including the moment its last growth
/// holds the old and new allocations. Frames are a `b8` record, a PTB64 group,
/// and an `r8` record whose every bit is set (one byte per bit plus its terminator), so the
/// bound does not depend on the input.
///
/// A line-framed text reader's transport buffer is excluded: it follows the longest line framed,
/// which the reader's `max_text_record_bytes` caps, so callers bound it through that cap.
/// [`RecordStreamReader::skip_records`] and [`RecordStreamReader::has_more_records`] decode up to
/// 64 records at a time through the same batch, except for `b8` and whole PTB64 groups, so a
/// caller that skips records passes that chunk size as well.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordStreamStorage {
    batch_bytes: u128,
    group_bytes: u128,
    transport_bytes: u128,
}

impl RecordStreamStorage {
    /// Bounds a reader of `format` records of `bits_per_record` bits whose batch reads request
    /// at most `max_batch_records` records.
    pub fn new(format: RecordFormat, bits_per_record: usize, max_batch_records: usize) -> Self {
        let width = bits_per_record as u128;
        let (group_bytes, frame_bytes) = match format {
            RecordFormat::B8 => (0, Some(bits_per_record.div_ceil(8) as u128)),
            RecordFormat::R8 => (0, Some(width.saturating_add(1))),
            RecordFormat::Ptb64 => {
                let planes = width.saturating_mul(size_of::<u64>() as u128);
                let rows = PackedShotBatch::storage_bytes(PTB64_GROUP_RECORDS, bits_per_record);
                (planes.saturating_add(rows), Some(planes))
            }
            RecordFormat::ZeroOne | RecordFormat::Hits | RecordFormat::Dets => (0, None),
        };
        Self {
            batch_bytes: PackedShotBatch::storage_bytes(max_batch_records, bits_per_record),
            group_bytes,
            transport_bytes: frame_bytes.map_or(0, transport_buffer_bytes),
        }
    }

    /// The whole bound in bytes.
    pub const fn bytes(self) -> u128 {
        self.batch_bytes
            .saturating_add(self.group_bytes)
            .saturating_add(self.transport_bytes)
    }

    /// The transport-buffer part of the bound; zero for line-framed text formats.
    pub const fn transport_bytes(self) -> u128 {
        self.transport_bytes
    }
}

/// The peak bytes of the transport buffer while frames of at most `frame_bytes` bytes span it.
///
/// A refill doubles the buffer only while the unconsumed bytes of one unfinished frame fill more
/// than half of it, so a buffer of `len` bytes grows only while `len < 2 * frame_bytes`; the
/// first refill allocates [`INITIAL_BUFFER_BYTES`]. A growth moves the contents into the larger
/// allocation, so the last one briefly holds both buffers.
fn transport_buffer_bytes(frame_bytes: u128) -> u128 {
    let initial = INITIAL_BUFFER_BYTES as u128;
    let mut len = initial;
    while len < frame_bytes.saturating_mul(2) {
        len = len.saturating_mul(2);
    }
    if len == initial {
        len
    } else {
        len.saturating_add(len / 2)
    }
}
