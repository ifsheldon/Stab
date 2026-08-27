//! Per-record streaming decode over byte transports.
//!
//! [`RecordStreamReader`] frames one record at a time out of an [`std::io::Read`] transport and
//! decodes each frame with the same component decoders the whole-buffer readers use, so streaming
//! and materialized decoding cannot drift. Component diagnostics are rebased to absolute input
//! offsets, keeping reported spans identical to whole-input decoding.

use std::io::Read;

use thiserror::Error;

use crate::{
    FormatError, RecordFormat, RecordResult,
    result_formats::DetsLayout,
    result_packed::{
        b8_length_multiple_error, b8_record_byte_width, decode_next_r8_record,
        extend_ptb64_group_words, fill_record_from_ptb64_words, ptb64_bytes_per_group,
        ptb64_length_multiple_error, ptb64_zero_width_count_error, unpack_b8_chunk_into,
    },
    result_streaming::{for_each_dets_record, for_each_record},
};

/// Bytes requested from the transport per refill; doubles while one record keeps spanning refills.
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// Failure surfaced by [`RecordStreamReader`]: either the byte transport failed or the framed
/// bytes violated the selected record-format contract.
#[derive(Debug, Error)]
pub enum RecordStreamReadError {
    /// The underlying byte transport failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The streamed bytes violated the record-format contract.
    #[error(transparent)]
    Format(#[from] FormatError),
}

#[derive(Debug)]
enum StreamDecoder {
    /// Line-framed text formats: `01`, HITS, and DETS.
    Text {
        format: RecordFormat,
        layout: DetsLayout,
    },
    /// Fixed-width byte-packed records.
    B8,
    /// Run-length records with data-dependent frame lengths.
    R8,
    /// 64-record measurement-major bit-plane groups, served one record at a time.
    Ptb64 {
        group_words: Vec<u64>,
        group_shots_served: usize,
    },
}

/// Decodes records one at a time from a byte transport without materializing the input.
///
/// The reader buffers only the bytes of the record frame currently being decoded (plus one read
/// chunk of lookahead), so memory stays bounded by the largest single record instead of the input
/// length. Text records are additionally bounded by the constructor's record-byte limit.
pub struct RecordStreamReader<R> {
    source: R,
    buffer: Vec<u8>,
    /// Cursor of the first unconsumed byte in `buffer`.
    start: usize,
    /// Absolute input offset of `buffer[0]`.
    absolute_base: usize,
    eof: bool,
    record: Vec<bool>,
    bits_per_record: usize,
    max_text_record_bytes: usize,
    decoder: StreamDecoder,
}

impl<R> std::fmt::Debug for RecordStreamReader<R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordStreamReader")
            .field("decoder", &self.decoder)
            .field("bits_per_record", &self.bits_per_record)
            .field("bytes_read", &self.bytes_read())
            .field("eof", &self.eof)
            .finish_non_exhaustive()
    }
}

impl<R> RecordStreamReader<R> {
    /// Total bytes fetched from the transport so far.
    pub fn bytes_read(&self) -> usize {
        self.absolute_base.saturating_add(self.buffer.len())
    }

    fn available(&self) -> usize {
        self.buffer.len().saturating_sub(self.start)
    }

    /// Absolute input offset of the next unconsumed byte, where the current record frame starts.
    fn record_offset(&self) -> RecordResult<usize> {
        self.absolute_base
            .checked_add(self.start)
            .ok_or_else(|| FormatError::invalid_data("streamed record byte offset overflowed"))
    }
}

impl<R: Read> RecordStreamReader<R> {
    /// Streams measurement-shaped records in the given per-record sample format.
    ///
    /// [`RecordFormat::Dets`] input uses a measurement-only layout, mirroring
    /// [`crate::read_records`]. `max_text_record_bytes` bounds one text record's framed bytes
    /// (including its newline) for the text formats and is ignored by the packed formats.
    pub fn measurements(
        source: R,
        format: RecordFormat,
        bits_per_record: usize,
        max_text_record_bytes: usize,
    ) -> Self {
        let decoder = match format {
            RecordFormat::B8 => StreamDecoder::B8,
            RecordFormat::R8 => StreamDecoder::R8,
            RecordFormat::ZeroOne | RecordFormat::Hits | RecordFormat::Dets => {
                StreamDecoder::Text {
                    format,
                    layout: DetsLayout::measurement_only(bits_per_record),
                }
            }
            RecordFormat::Ptb64 => StreamDecoder::Ptb64 {
                group_words: Vec::new(),
                group_shots_served: 64,
            },
        };
        Self::with_decoder(source, decoder, bits_per_record, max_text_record_bytes)
    }

    /// Streams DETS records resolved against an explicit typed namespace layout.
    pub fn dets(source: R, layout: DetsLayout, max_text_record_bytes: usize) -> Self {
        Self::with_decoder(
            source,
            StreamDecoder::Text {
                format: RecordFormat::Dets,
                layout,
            },
            layout.total_bits(),
            max_text_record_bytes,
        )
    }

    /// Streams ptb64 records, decoding one complete 64-record group at a time.
    pub fn ptb64(source: R, bits_per_record: usize) -> Self {
        Self::with_decoder(
            source,
            StreamDecoder::Ptb64 {
                group_words: Vec::new(),
                group_shots_served: 64,
            },
            bits_per_record,
            usize::MAX,
        )
    }

    fn with_decoder(
        source: R,
        decoder: StreamDecoder,
        bits_per_record: usize,
        max_text_record_bytes: usize,
    ) -> Self {
        Self {
            source,
            buffer: Vec::new(),
            start: 0,
            absolute_base: 0,
            eof: false,
            record: vec![false; bits_per_record],
            bits_per_record,
            max_text_record_bytes,
            decoder,
        }
    }

    /// Decodes the next record, or returns `None` at a clean end of input.
    ///
    /// The returned slice borrows the reader's reusable record buffer and is valid until the next
    /// call.
    pub fn next_record(&mut self) -> Result<Option<&[bool]>, RecordStreamReadError> {
        match &self.decoder {
            StreamDecoder::Text { .. } => self.next_text_record(),
            StreamDecoder::B8 => self.next_b8_record(),
            StreamDecoder::R8 => self.next_r8_record(),
            StreamDecoder::Ptb64 { .. } => self.next_ptb64_record(),
        }
    }

    /// Returns the next raw byte-packed record frame without expanding it into dense bits.
    ///
    /// This representation-preserving path is available only when the reader was constructed for
    /// [`RecordFormat::B8`]. The returned frame borrows the reader's input buffer and remains valid
    /// until the next mutable reader operation. Padding bits in the final byte are returned exactly
    /// as supplied; semantic consumers that emit canonical B8 must clear bits beyond the declared
    /// record width.
    pub fn next_b8_packed_record(&mut self) -> Result<Option<&[u8]>, RecordStreamReadError> {
        if !matches!(&self.decoder, StreamDecoder::B8) {
            return Err(FormatError::invalid_data(
                "packed b8 record access requires a b8 stream reader",
            )
            .into());
        }
        let Some((start, end)) = self.next_b8_frame_range()? else {
            return Ok(None);
        };
        self.buffer.get(start..end).map(Some).ok_or_else(|| {
            FormatError::invalid_data("b8 record frame escaped the stream buffer").into()
        })
    }

    fn next_text_record(&mut self) -> Result<Option<&[bool]>, RecordStreamReadError> {
        loop {
            let Some(line_len) = self.frame_text_line()? else {
                return Ok(None);
            };
            let record_offset = self.record_offset()?;
            let line_start = self.start;
            let line_end = line_start.checked_add(line_len).ok_or_else(|| {
                FormatError::invalid_data("text record frame byte range overflowed")
            })?;
            let produced = {
                let line = self.buffer.get(line_start..line_end).ok_or_else(|| {
                    FormatError::invalid_data("text record frame escaped the stream buffer")
                })?;
                let StreamDecoder::Text { format, layout } = &self.decoder else {
                    return Err(internal_decoder_mismatch().into());
                };
                let mut produced = false;
                let record = &mut self.record;
                let visit = |bits: &[bool]| {
                    if produced {
                        return Err(FormatError::invalid_data(
                            "text record frame decoded into multiple records",
                        ));
                    }
                    if bits.len() != record.len() {
                        return Err(FormatError::invalid_data(
                            "decoded record width disagreed with the stream record buffer",
                        ));
                    }
                    record.copy_from_slice(bits);
                    produced = true;
                    Ok(())
                };
                let decoded = match format {
                    RecordFormat::Dets => for_each_dets_record(line, *layout, visit),
                    format => for_each_record(line, *format, layout.total_bits(), visit),
                };
                decoded.map_err(|error| error.with_span_offset(record_offset))?;
                produced
            };
            self.start = line_end;
            if produced {
                return Ok(Some(&self.record));
            }
            // A whitespace-only DETS line frames zero records; keep scanning.
        }
    }

    /// Frames the next line (through its newline, or the unterminated tail at end of input) and
    /// returns its length, without consuming it.
    fn frame_text_line(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
        let mut scanned = 0usize;
        loop {
            let unscanned = self
                .buffer
                .get(self.start.saturating_add(scanned)..)
                .unwrap_or_default();
            if let Some(position) = unscanned.iter().position(|byte| *byte == b'\n') {
                let line_len = scanned.saturating_add(position).saturating_add(1);
                if line_len > self.max_text_record_bytes {
                    return Err(self.text_record_limit_error());
                }
                return Ok(Some(line_len));
            }
            scanned = self.available();
            if scanned > self.max_text_record_bytes {
                return Err(self.text_record_limit_error());
            }
            if self.eof {
                if scanned == 0 {
                    return Ok(None);
                }
                return Ok(Some(scanned));
            }
            self.refill()?;
        }
    }

    fn text_record_limit_error(&self) -> RecordStreamReadError {
        FormatError::invalid_data(format!(
            "text record exceeded the {}-byte streaming record limit before its newline",
            self.max_text_record_bytes
        ))
        .into()
    }

    fn next_b8_record(&mut self) -> Result<Option<&[bool]>, RecordStreamReadError> {
        let Some((start, end)) = self.next_b8_frame_range()? else {
            return Ok(None);
        };
        let chunk = self.buffer.get(start..end).ok_or_else(|| {
            FormatError::invalid_data("b8 record frame escaped the stream buffer")
        })?;
        unpack_b8_chunk_into(chunk, &mut self.record);
        Ok(Some(&self.record))
    }

    fn next_b8_frame_range(&mut self) -> Result<Option<(usize, usize)>, RecordStreamReadError> {
        let bytes_per_record = b8_record_byte_width(self.bits_per_record)?;
        self.fill_at_least(bytes_per_record)?;
        if self.available() == 0 {
            return Ok(None);
        }
        if self.available() < bytes_per_record {
            return Err(b8_length_multiple_error(self.bytes_read(), bytes_per_record).into());
        }
        let end = self
            .start
            .checked_add(bytes_per_record)
            .ok_or_else(|| FormatError::invalid_data("b8 record frame byte range overflowed"))?;
        let start = self.start;
        self.buffer.get(start..end).ok_or_else(|| {
            FormatError::invalid_data("b8 record frame escaped the stream buffer")
        })?;
        self.start = end;
        Ok(Some((start, end)))
    }

    fn next_r8_record(&mut self) -> Result<Option<&[bool]>, RecordStreamReadError> {
        loop {
            if self.available() == 0 {
                if self.eof {
                    return Ok(None);
                }
                self.refill()?;
                continue;
            }
            let record_offset = self.record_offset()?;
            self.record.fill(false);
            let mut consumed = 0usize;
            let decoded = {
                let frame = self.buffer.get(self.start..).unwrap_or_default();
                let record = &mut self.record;
                let bits_per_record = self.bits_per_record;
                decode_next_r8_record(frame, bits_per_record, &mut consumed, |bit_index| {
                    let Some(bit) = record.get_mut(bit_index) else {
                        return Err(FormatError::invalid_data(format!(
                            "r8 hit index {bit_index} exceeds record width {bits_per_record}"
                        )));
                    };
                    *bit = true;
                    Ok(())
                })
            };
            match decoded {
                Ok(true) => {
                    self.start = self.start.checked_add(consumed).ok_or_else(|| {
                        FormatError::invalid_data("r8 record frame byte range overflowed")
                    })?;
                    return Ok(Some(&self.record));
                }
                // An empty frame is impossible here: `available() > 0` was checked above.
                Ok(false) => return Ok(None),
                Err(error)
                    if error.code() == crate::FormatErrorCode::UnexpectedEndOfInput
                        && !self.eof =>
                {
                    // The record spans the buffered window; fetch more bytes and redecode it
                    // from its start. Refills grow geometrically, bounding total redecode work
                    // by a constant factor of the record length.
                    self.refill()?;
                }
                Err(error) => return Err(error.with_span_offset(record_offset).into()),
            }
        }
    }

    fn next_ptb64_record(&mut self) -> Result<Option<&[bool]>, RecordStreamReadError> {
        if self.bits_per_record == 0 {
            return Err(ptb64_zero_width_count_error().into());
        }
        let needs_group = match &self.decoder {
            StreamDecoder::Ptb64 {
                group_words,
                group_shots_served,
            } => *group_shots_served >= 64 || group_words.is_empty(),
            _ => return Err(internal_decoder_mismatch().into()),
        };
        if needs_group {
            let bytes_per_group = ptb64_bytes_per_group(self.bits_per_record)?;
            self.fill_at_least(bytes_per_group)?;
            if self.available() == 0 {
                return Ok(None);
            }
            if self.available() < bytes_per_group {
                return Err(ptb64_length_multiple_error(self.bytes_read(), bytes_per_group).into());
            }
            let end = self.start.checked_add(bytes_per_group).ok_or_else(|| {
                FormatError::invalid_data("ptb64 group frame byte range overflowed")
            })?;
            let group_bytes = self.buffer.get(self.start..end).ok_or_else(|| {
                FormatError::invalid_data("ptb64 group frame escaped the stream buffer")
            })?;
            let StreamDecoder::Ptb64 {
                group_words,
                group_shots_served,
            } = &mut self.decoder
            else {
                return Err(internal_decoder_mismatch().into());
            };
            extend_ptb64_group_words(group_bytes, group_words);
            *group_shots_served = 0;
            self.start = end;
        }
        let StreamDecoder::Ptb64 {
            group_words,
            group_shots_served,
        } = &mut self.decoder
        else {
            return Err(internal_decoder_mismatch().into());
        };
        fill_record_from_ptb64_words(group_words, *group_shots_served, &mut self.record);
        *group_shots_served += 1;
        Ok(Some(&self.record))
    }

    fn fill_at_least(&mut self, wanted: usize) -> Result<(), RecordStreamReadError> {
        while self.available() < wanted && !self.eof {
            self.refill()?;
        }
        Ok(())
    }

    /// Compacts consumed bytes out of the buffer and fetches the next transport chunk.
    ///
    /// The requested chunk doubles with the amount of unconsumed data already buffered, so a
    /// record spanning many refills costs amortized linear framing work.
    fn refill(&mut self) -> Result<(), RecordStreamReadError> {
        if self.eof {
            return Ok(());
        }
        if self.start > 0 {
            self.absolute_base = self.absolute_base.checked_add(self.start).ok_or_else(|| {
                FormatError::invalid_data("streamed input byte offset overflowed")
            })?;
            self.buffer.drain(..self.start);
            self.start = 0;
        }
        let old_len = self.buffer.len();
        let chunk = old_len.max(READ_CHUNK_BYTES);
        let new_len = old_len
            .checked_add(chunk)
            .ok_or_else(|| FormatError::invalid_data("streamed record buffer length overflowed"))?;
        self.buffer.try_reserve(chunk).map_err(|error| {
            FormatError::invalid_data(format!(
                "record stream could not reserve {chunk} buffered input bytes: {error}"
            ))
        })?;
        self.buffer.resize(new_len, 0);
        let read = loop {
            let window = self.buffer.get_mut(old_len..).unwrap_or_default();
            match self.source.read(window) {
                Ok(read) => break read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    self.buffer.truncate(old_len);
                    return Err(error.into());
                }
            }
        };
        self.buffer.truncate(old_len.saturating_add(read));
        if read == 0 {
            self.eof = true;
        }
        Ok(())
    }
}

fn internal_decoder_mismatch() -> FormatError {
    FormatError::invalid_data("record stream decoder state disagreed with its selected format")
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used,
        reason = "streaming reader tests use direct fixture assertions for compact diagnostics"
    )]

    use super::*;
    use crate::{read_ptb64_records_all, read_records, write_ptb64_records_checked, write_records};

    const TEST_TEXT_LIMIT: usize = 1024 * 1024;

    /// Feeds bytes to the reader a few bytes per `read` call to exercise refill boundaries.
    struct Trickle<'a> {
        bytes: &'a [u8],
        offset: usize,
        step: usize,
    }

    impl<'a> Trickle<'a> {
        fn new(bytes: &'a [u8], step: usize) -> Self {
            Self {
                bytes,
                offset: 0,
                step,
            }
        }
    }

    impl Read for Trickle<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let remaining = &self.bytes[self.offset..];
            let step = self.step.min(remaining.len()).min(buffer.len());
            buffer[..step].copy_from_slice(&remaining[..step]);
            self.offset += step;
            Ok(step)
        }
    }

    fn drain_measurements(
        input: &[u8],
        format: RecordFormat,
        width: usize,
        step: usize,
    ) -> Result<Vec<Vec<bool>>, RecordStreamReadError> {
        let mut reader = RecordStreamReader::measurements(
            Trickle::new(input, step),
            format,
            width,
            TEST_TEXT_LIMIT,
        );
        let mut records = Vec::new();
        while let Some(record) = reader.next_record()? {
            records.push(record.to_vec());
        }
        Ok(records)
    }

    fn expect_format_error(result: Result<Vec<Vec<bool>>, RecordStreamReadError>) -> FormatError {
        match result {
            Err(RecordStreamReadError::Format(error)) => error,
            other => panic!("expected a format error, got {other:?}"),
        }
    }

    #[test]
    fn streamed_records_match_whole_buffer_readers_for_every_sample_format() {
        let records = vec![
            vec![true, false, true, false, false, true, false, false, true],
            vec![false, true, false, true, false, false, true, false, false],
            vec![
                false, false, false, false, false, false, false, false, false,
            ],
            vec![true; 9],
        ];
        for format in [
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Dets,
        ] {
            let input = write_records(&records, format).expect("encode records");
            let expected = read_records(&input, format, 9).expect("whole-buffer read");
            for step in [1, 2, 7, input.len().max(1)] {
                assert_eq!(
                    drain_measurements(&input, format, 9, step).expect("streamed read"),
                    expected,
                    "{format:?} step {step}"
                );
            }
        }
    }

    #[test]
    fn streamed_b8_packed_records_preserve_frames_across_refills() {
        let input = [0xab, 0x01, 0x34, 0x02, 0xff, 0x00];
        for step in [1, 2, input.len()] {
            let mut reader = RecordStreamReader::measurements(
                Trickle::new(&input, step),
                RecordFormat::B8,
                9,
                TEST_TEXT_LIMIT,
            );
            let mut records = Vec::new();
            while let Some(record) = reader
                .next_b8_packed_record()
                .expect("streamed packed b8 record")
            {
                records.push(record.to_vec());
            }
            assert_eq!(
                records,
                vec![vec![0xab, 0x01], vec![0x34, 0x02], vec![0xff, 0x00]],
                "step {step}"
            );
        }
    }

    #[test]
    fn generic_measurement_stream_reads_ptb64_groups() {
        let records = (0usize..64)
            .map(|shot| {
                (0usize..9)
                    .map(|bit| (shot * 7 + bit * 11).is_multiple_of(13))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let input = write_records(&records, RecordFormat::Ptb64).expect("encode ptb64");

        for step in [1, 7, input.len()] {
            assert_eq!(
                drain_measurements(&input, RecordFormat::Ptb64, 9, step).expect("stream ptb64"),
                records,
                "step {step}"
            );
        }
    }

    #[test]
    fn streamed_b8_packed_records_keep_validation_and_format_identity() {
        let mut truncated = RecordStreamReader::measurements(
            Trickle::new(&[0xab, 0x01, 0x34], 1),
            RecordFormat::B8,
            9,
            TEST_TEXT_LIMIT,
        );
        assert_eq!(
            truncated
                .next_b8_packed_record()
                .expect("complete leading record")
                .expect("leading record"),
            &[0xab, 0x01]
        );
        let expected = read_records(&[0xab, 0x01, 0x34], RecordFormat::B8, 9)
            .expect_err("truncated whole-buffer b8 input");
        let actual = match truncated
            .next_b8_packed_record()
            .expect_err("truncated streamed b8 input")
        {
            RecordStreamReadError::Format(error) => error,
            other => panic!("expected format error, got {other:?}"),
        };
        assert_eq!(actual, expected);

        let mut text = RecordStreamReader::measurements(
            Trickle::new(b"0\n", 1),
            RecordFormat::ZeroOne,
            1,
            TEST_TEXT_LIMIT,
        );
        assert!(matches!(
            text.next_b8_packed_record(),
            Err(RecordStreamReadError::Format(_))
        ));

        let mut zero_width = RecordStreamReader::measurements(
            Trickle::new(&[0xff], 1),
            RecordFormat::B8,
            0,
            TEST_TEXT_LIMIT,
        );
        assert!(matches!(
            zero_width.next_b8_packed_record(),
            Err(RecordStreamReadError::Format(error))
                if error.code() == crate::FormatErrorCode::InvalidRecordWidth
        ));
    }

    #[test]
    fn streamed_text_records_accept_crlf_blank_dets_lines_and_eof_tails() {
        assert_eq!(
            drain_measurements(b"01\r\n01\r\n", RecordFormat::ZeroOne, 2, 3).unwrap(),
            vec![vec![false, true], vec![false, true]]
        );
        assert_eq!(
            drain_measurements(b"3\r\n1\r\n", RecordFormat::Hits, 4, 1).unwrap(),
            vec![
                vec![false, false, false, true],
                vec![false, true, false, false],
            ]
        );
        assert_eq!(
            drain_measurements(
                b"shot M3\r\n\r\n\n   shot M1\r\n\n",
                RecordFormat::Dets,
                4,
                2
            )
            .unwrap(),
            vec![
                vec![false, false, false, true],
                vec![false, true, false, false],
            ]
        );
        // A DETS record may end at end of input without a newline.
        assert_eq!(
            drain_measurements(b"shot M1", RecordFormat::Dets, 2, 3).unwrap(),
            vec![vec![false, true]]
        );
    }

    #[test]
    fn streamed_dets_records_resolve_explicit_typed_layouts() {
        let layout = DetsLayout::try_new(1, 2, 1).expect("layout");
        let mut reader = RecordStreamReader::dets(
            Trickle::new(b"shot M0 D1 L0\nshot\n", 2),
            layout,
            TEST_TEXT_LIMIT,
        );
        let mut records = Vec::new();
        while let Some(record) = reader.next_record().expect("streamed dets") {
            records.push(record.to_vec());
        }
        assert_eq!(records, vec![vec![true, false, true, true], vec![false; 4]]);
    }

    #[test]
    fn streamed_format_errors_are_byte_identical_to_whole_buffer_errors() {
        // Malformed tails and mid-stream violations, per format. The whole-buffer readers
        // validate packed input lengths before decoding any record, so for those formats only
        // the error value (not the record prefix) is compared.
        let cases: &[(RecordFormat, &[u8], usize)] = &[
            (RecordFormat::ZeroOne, b"101\n1", 3),
            (RecordFormat::ZeroOne, b"10\n", 3),
            (RecordFormat::ZeroOne, b"1x1\n", 3),
            (RecordFormat::ZeroOne, b"101\r", 3),
            (RecordFormat::B8, &[0xAB], 9),
            (RecordFormat::B8, &[0xAB, 0x01, 0x02], 9),
            (RecordFormat::R8, &[4], 3),
            (RecordFormat::R8, &[255], 300),
            (RecordFormat::R8, &[1, 200], 3),
            (RecordFormat::Hits, b"100,1\n", 3),
            (RecordFormat::Hits, b"18446744073709551616\n", 3),
            (RecordFormat::Hits, b"0\n3", 3),
            (RecordFormat::Dets, b"D2\n", 3),
            (RecordFormat::Dets, b"shot X2\n", 3),
            (RecordFormat::Dets, b"shot M0\nshot M9\n", 3),
        ];
        for (format, input, width) in cases {
            let expected =
                read_records(input, *format, *width).expect_err("whole-buffer rejection");
            for step in [1, 2, input.len()] {
                let actual = expect_format_error(drain_measurements(input, *format, *width, step));
                assert_eq!(actual, expected, "{format:?} {input:?} step {step}");
            }
        }
    }

    #[test]
    fn streamed_ptb64_groups_match_whole_buffer_reader_and_errors() {
        let records = (0..128)
            .map(|shot_index| {
                (0..17)
                    .map(|bit_index| (shot_index * 7 + bit_index * 11) % 13 == 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let input = write_ptb64_records_checked(&records).expect("ptb64 fixture");
        let expected = read_ptb64_records_all(&input, 17).expect("whole-buffer ptb64");
        for step in [1, 7, input.len()] {
            let mut reader = RecordStreamReader::ptb64(Trickle::new(&input, step), 17);
            let mut streamed = Vec::new();
            while let Some(record) = reader.next_record().expect("streamed ptb64") {
                streamed.push(record.to_vec());
            }
            assert_eq!(streamed, expected, "step {step}");
        }

        // Truncated trailing group: identical error value to the whole-buffer reader.
        let truncated = &input[..input.len() - 1];
        let expected_error = read_ptb64_records_all(truncated, 17).expect_err("trailing bytes");
        let mut reader = RecordStreamReader::ptb64(Trickle::new(truncated, 5), 17);
        let mut streamed = 0usize;
        let error = loop {
            match reader.next_record() {
                Ok(Some(_)) => streamed += 1,
                Ok(None) => panic!("truncated ptb64 input must not end cleanly"),
                Err(RecordStreamReadError::Format(error)) => break error,
                Err(other) => panic!("unexpected transport failure: {other:?}"),
            }
        };
        assert_eq!(error, expected_error);
        // The complete leading group still streams before the trailing group is rejected.
        assert_eq!(streamed, 64);

        // Zero-width and zero-length behaviors.
        let mut reader = RecordStreamReader::ptb64(Trickle::new(&[], 1), 0);
        assert!(matches!(
            reader.next_record(),
            Err(RecordStreamReadError::Format(_))
        ));
        let mut reader = RecordStreamReader::ptb64(Trickle::new(&[], 1), 3);
        assert!(reader.next_record().expect("empty input").is_none());
    }

    #[test]
    fn streamed_reader_reports_frame_relative_spans_rebased_to_absolute_offsets() {
        // The second record's malformed byte sits at absolute offset 5.
        let input = b"101\n1x1\n";
        let expected = read_records(input, RecordFormat::ZeroOne, 3).expect_err("bad byte");
        let actual = expect_format_error(drain_measurements(input, RecordFormat::ZeroOne, 3, 2));
        assert_eq!(actual, expected);
        assert_eq!(actual.span().expect("span").byte_start(), 5);

        // r8: overshoot inside the second record keeps its absolute one-byte span.
        let input: &[u8] = &[1, 1, 200];
        let expected = read_records(input, RecordFormat::R8, 3).expect_err("overshoot");
        let actual = expect_format_error(drain_measurements(input, RecordFormat::R8, 3, 1));
        assert_eq!(actual, expected);
        assert_eq!(actual.span().expect("span").byte_start(), 2);
    }

    #[test]
    fn streamed_records_before_a_mid_stream_error_match_whole_buffer_prefixes() {
        let input = b"101\n010\nbad\n";
        let mut reader = RecordStreamReader::measurements(
            Trickle::new(input, 3),
            RecordFormat::ZeroOne,
            3,
            TEST_TEXT_LIMIT,
        );
        let mut streamed = Vec::new();
        let error = loop {
            match reader.next_record() {
                Ok(Some(record)) => streamed.push(record.to_vec()),
                Ok(None) => panic!("malformed input must not end cleanly"),
                Err(RecordStreamReadError::Format(error)) => break error,
                Err(other) => panic!("unexpected transport failure: {other:?}"),
            }
        };
        assert_eq!(
            streamed,
            vec![vec![true, false, true], vec![false, true, false]]
        );
        let expected = read_records(input, RecordFormat::ZeroOne, 3).expect_err("bad byte");
        assert_eq!(error, expected);
    }

    #[test]
    fn streamed_text_records_reject_frames_beyond_the_record_byte_limit() {
        let mut reader = RecordStreamReader::measurements(
            Trickle::new(b"0101010101", 1),
            RecordFormat::ZeroOne,
            10,
            4,
        );
        let error = reader.next_record().expect_err("over-limit record");
        assert!(matches!(error, RecordStreamReadError::Format(_)));
        assert!(error.to_string().contains("4-byte streaming record limit"));
    }

    #[test]
    fn streamed_reader_surfaces_transport_failures_as_io_errors() {
        struct FailingTransport;
        impl Read for FailingTransport {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("intentional transport stop"))
            }
        }
        let mut reader = RecordStreamReader::measurements(
            FailingTransport,
            RecordFormat::ZeroOne,
            1,
            TEST_TEXT_LIMIT,
        );
        assert!(matches!(
            reader.next_record(),
            Err(RecordStreamReadError::Io(_))
        ));
    }

    #[test]
    fn streamed_reader_tracks_total_bytes_read_for_transport_adapters() {
        let records = vec![vec![true, false]; 64];
        let input = write_ptb64_records_checked(&records).expect("ptb64 fixture");
        let mut reader = RecordStreamReader::ptb64(Trickle::new(&input, 3), 2);
        while reader.next_record().expect("streamed ptb64").is_some() {}
        assert_eq!(reader.bytes_read(), input.len());
    }
}
