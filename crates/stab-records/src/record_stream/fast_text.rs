//! Single-pass fast paths for canonical HITS and DETS lines.
//!
//! Each fast path frames and decodes one line in one byte loop, writing set bits straight into
//! packed row words. It only accepts the canonical spelling of a record that the shared grammar
//! in `result_text` would accept with the same row, terminated by `\n` or `\r\n` within
//! `max_line_bytes`, and declines everything else (malformed bytes, out-of-range or overflowing
//! indexes, unusual separators, leading whitespace, oversized lines). A declined line leaves the
//! row unspecified and is redecoded from its start by the full grammar, which owns every
//! diagnostic, so the fast paths cannot change an accepted value or an error.
//!
//! A line that runs past the buffered bytes reports [`FastLine::Incomplete`] with the offset of
//! the first token it has not applied; after the caller buffers more of the same line it resumes
//! there with the row as left, so a line spanning many refills is still parsed once. A resume
//! offset that precedes the first token restarts the line, rechecking its prefix.

use super::HitsDuplicates;
use crate::DetsLayout;

const WORD_BITS: usize = u64::BITS as usize;

/// Outcome of one fast-path attempt over the buffered bytes of one line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FastLine {
    /// One record was decoded into the row; the line, including its newline, has this length.
    Record(usize),
    /// The buffered bytes ended inside a canonical line; resume at this line offset once more
    /// bytes of the line are buffered. Every token before it is applied to the row.
    Incomplete(usize),
    /// The line must be judged by the full grammar.
    Declined,
}

/// Where a line terminator check landed.
enum LineEnd {
    Complete(usize),
    Truncated,
    Invalid,
}

/// Decodes one canonical HITS line (`\n`, or comma-separated decimal indexes then `\n` or
/// `\r\n`) into `words`, starting fresh unless `resume` points past the line start.
pub(super) fn hits_line(
    input: &[u8],
    width: usize,
    duplicates: HitsDuplicates,
    max_line_bytes: usize,
    words: &mut [u64],
    resume: Option<usize>,
) -> FastLine {
    let input = input.get(..max_line_bytes).unwrap_or(input);
    let mut offset = match resume {
        Some(offset) if offset > 0 => offset,
        _ => {
            words.fill(0);
            match line_end(input, 0) {
                LineEnd::Complete(len) => return FastLine::Record(len),
                LineEnd::Truncated => return FastLine::Incomplete(0),
                LineEnd::Invalid => 0,
            }
        }
    };
    loop {
        let token = offset;
        let (value, next) = digit_run(input, offset);
        // Apply a token only once the byte after it is buffered, so a resumed parse never
        // applies a digit run twice.
        let line_len = match input.get(next) {
            None => return FastLine::Incomplete(token),
            Some(b',') => None,
            Some(b'\n') => Some(next + 1),
            Some(b'\r') => match input.get(next + 1) {
                Some(b'\n') => Some(next + 2),
                Some(_) => return FastLine::Declined,
                None => return FastLine::Incomplete(token),
            },
            Some(_) => return FastLine::Declined,
        };
        let Some(index) = checked_index(value, next - token, width) else {
            return FastLine::Declined;
        };
        let Some(word) = words.get_mut(index / WORD_BITS) else {
            return FastLine::Declined;
        };
        let bit = 1_u64 << (index % WORD_BITS);
        match duplicates {
            HitsDuplicates::Toggle => *word ^= bit,
            HitsDuplicates::Set => *word |= bit,
        }
        match line_len {
            Some(len) => return FastLine::Record(len),
            None => offset = next + 1,
        }
    }
}

/// Decodes one canonical DETS line (`shot`, then ` M<i>`, ` D<i>`, or ` L<i>` tokens, then `\n`
/// or `\r\n`) into `words`, resolving each token against `layout`, starting fresh unless
/// `resume` points past the `shot` prefix.
pub(super) fn dets_line(
    input: &[u8],
    layout: DetsLayout,
    max_line_bytes: usize,
    words: &mut [u64],
    resume: Option<usize>,
) -> FastLine {
    const PREFIX: &[u8] = b"shot";
    let input = input.get(..max_line_bytes).unwrap_or(input);
    let mut offset = match resume {
        Some(offset) if offset >= PREFIX.len() => offset,
        _ => {
            match input.get(..PREFIX.len()) {
                Some(prefix) if prefix == PREFIX => {}
                Some(_) => return FastLine::Declined,
                None if PREFIX.starts_with(input) => return FastLine::Incomplete(0),
                None => return FastLine::Declined,
            }
            words.fill(0);
            PREFIX.len()
        }
    };
    let observable_offset = layout.measurements() + layout.detectors();
    loop {
        let token = offset;
        match input.get(offset) {
            None => return FastLine::Incomplete(token),
            Some(b' ') => offset += 1,
            Some(_) => {
                return match line_end(input, offset) {
                    LineEnd::Complete(len) => FastLine::Record(len),
                    LineEnd::Truncated => FastLine::Incomplete(token),
                    LineEnd::Invalid => FastLine::Declined,
                };
            }
        }
        let (namespace_offset, namespace_width) = match input.get(offset) {
            None => return FastLine::Incomplete(token),
            Some(b'M') => (0, layout.measurements()),
            Some(b'D') => (layout.measurements(), layout.detectors()),
            Some(b'L') => (observable_offset, layout.observables()),
            Some(_) => return FastLine::Declined,
        };
        let (value, next) = digit_run(input, offset + 1);
        if input.get(next).is_none() {
            return FastLine::Incomplete(token);
        }
        let Some(index) = checked_index(value, next - offset - 1, namespace_width) else {
            return FastLine::Declined;
        };
        // `index < namespace_width`, and the namespaces partition the layout's checked total.
        let bit_index = namespace_offset + index;
        let Some(word) = words.get_mut(bit_index / WORD_BITS) else {
            return FastLine::Declined;
        };
        *word |= 1_u64 << (bit_index % WORD_BITS);
        offset = next;
    }
}

/// Classifies the bytes at `offset` as a `\n` or `\r\n` terminator.
fn line_end(input: &[u8], offset: usize) -> LineEnd {
    match input.get(offset..) {
        Some([b'\n', ..]) => LineEnd::Complete(offset + 1),
        Some([b'\r', b'\n', ..]) => LineEnd::Complete(offset + 2),
        Some([] | [b'\r']) => LineEnd::Truncated,
        _ => LineEnd::Invalid,
    }
}

/// Longest digit run the fast paths accept; any 19-digit decimal fits in `u64`. Longer runs
/// (only possible with leading zeros for an in-range index) go to the full grammar.
const MAX_DIGITS: usize = 19;

/// Scans the decimal digit run at `offset`, returning its wrapped value and the offset just past
/// it. The value is exact whenever the run has at most [`MAX_DIGITS`] digits.
///
/// The scan stops after `MAX_DIGITS + 1` digits, which callers decline, so resuming a line at
/// a token start rescans a bounded number of bytes and a hostile digit run stays linear.
fn digit_run(input: &[u8], offset: usize) -> (u64, usize) {
    let mut value = 0_u64;
    let mut end = offset;
    let scan_end = offset.saturating_add(MAX_DIGITS + 1);
    while let Some(byte) = input.get(end).filter(|_| end < scan_end) {
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            break;
        }
        value = value.wrapping_mul(10).wrapping_add(u64::from(digit));
        end += 1;
    }
    (value, end)
}

/// Accepts a complete digit run of `len` digits as an index below `bound`.
fn checked_index(value: u64, len: usize, bound: usize) -> Option<usize> {
    if len == 0 || len > MAX_DIGITS {
        return None;
    }
    usize::try_from(value).ok().filter(|index| *index < bound)
}

#[cfg(test)]
mod tests;
