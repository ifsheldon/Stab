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
/// [`RecordStreamReader::read_batch`] (at least one), the PTB64 bit-plane and transposed-row
/// group, the hits of the first `r8` record (decoded before the batch exists, in at most one row
/// of storage beside it), and for the packed formats (`b8`, `r8`, and PTB64) the transport buffer
/// at its peak, including the moment its last growth holds the old and new allocations. A `b8`
/// record or a PTB64 group spans the buffer whole, so its buffer grows around one frame; `r8`
/// records decode as their bytes arrive, so its buffer keeps its initial size whatever the
/// record width. The bound does not depend on the input.
///
/// A line-framed text reader's transport buffer is excluded: it follows the longest line framed,
/// which the reader's `max_text_record_bytes` caps, so callers bound it through that cap with
/// [`Self::text_transport_bytes`] or [`Self::max_text_record_bytes`].
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
        let (group_bytes, transport_bytes) = match format {
            RecordFormat::B8 => (
                0,
                transport_buffer_bytes(bits_per_record.div_ceil(8) as u128),
            ),
            // The first record's hits peak below two rows while it decodes and below one row
            // beside the batch, which holds at least one row, once it completes.
            RecordFormat::R8 => (
                PackedShotBatch::storage_bytes(1, bits_per_record),
                INITIAL_BUFFER_BYTES as u128,
            ),
            RecordFormat::Ptb64 => {
                let planes = width.saturating_mul(size_of::<u64>() as u128);
                let rows = PackedShotBatch::storage_bytes(PTB64_GROUP_RECORDS, bits_per_record);
                (planes.saturating_add(rows), transport_buffer_bytes(planes))
            }
            RecordFormat::ZeroOne | RecordFormat::Hits | RecordFormat::Dets => (0, 0),
        };
        Self {
            batch_bytes: PackedShotBatch::storage_bytes(max_batch_records.max(1), bits_per_record),
            group_bytes,
            transport_bytes,
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

    /// Upper bound on the transport buffer of a line-framed text reader (`01`, HITS, or DETS)
    /// whose `max_text_record_bytes` is `max_text_record_bytes`.
    ///
    /// The buffer grows only up to one byte past the record limit. Below the initial size it is
    /// allocated at that limit once; otherwise its last growth briefly holds the old allocation,
    /// smaller than the limit, beside the new one, so the peak is below twice the limit.
    pub const fn text_transport_bytes(max_text_record_bytes: usize) -> u128 {
        let limit = max_text_record_bytes as u128 + 1;
        if limit <= INITIAL_BUFFER_BYTES as u128 {
            limit
        } else {
            2 * limit - 1
        }
    }

    /// The largest `max_text_record_bytes` whose [`Self::text_transport_bytes`] fits within
    /// `budget_bytes`, or zero when even an empty limit would not.
    pub fn max_text_record_bytes(budget_bytes: u128) -> usize {
        let initial = INITIAL_BUFFER_BYTES as u128;
        let cap = if budget_bytes <= initial {
            // A limit within the initial size is allocated once, at the limit.
            budget_bytes.saturating_sub(1)
        } else {
            (budget_bytes.saturating_sub(1) / 2).max(initial - 1)
        };
        usize::try_from(cap).unwrap_or(usize::MAX / 2)
    }
}

/// The most bytes a line-framed text reader's transport buffer holds: one byte past its record
/// limit, enough to frame any line of at most that many bytes and to see that one is longer.
pub(super) const fn text_buffer_limit(max_text_record_bytes: usize) -> usize {
    max_text_record_bytes.saturating_add(1)
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
