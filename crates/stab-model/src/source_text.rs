use crate::{ByteSpan, advanced::byte_span_from_valid_range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceSlice<'a> {
    text: &'a str,
    byte_start: usize,
}

impl<'a> SourceSlice<'a> {
    pub(crate) const fn new(text: &'a str, byte_start: usize) -> Self {
        Self { text, byte_start }
    }

    pub(crate) const fn text(self) -> &'a str {
        self.text
    }

    pub(crate) const fn byte_start(self) -> usize {
        self.byte_start
    }

    pub(crate) const fn byte_end(self) -> usize {
        self.byte_start + self.text.len()
    }

    pub(crate) fn span(self) -> ByteSpan {
        byte_span_from_valid_range(self.byte_start, self.text.len())
    }

    pub(crate) fn end_span(self) -> ByteSpan {
        byte_span_from_valid_range(self.byte_start + self.text.len(), 0)
    }

    pub(crate) fn subspan(self, relative_start: usize, byte_length: usize) -> Option<ByteSpan> {
        let relative_end = relative_start.checked_add(byte_length)?;
        if relative_end > self.text.len()
            || !self.text.is_char_boundary(relative_start)
            || !self.text.is_char_boundary(relative_end)
        {
            return None;
        }
        ByteSpan::try_new(self.byte_start.checked_add(relative_start)?, byte_length)
    }

    pub(crate) fn trim_ascii_start(self) -> Self {
        let trimmed = self.text.trim_ascii_start();
        Self {
            text: trimmed,
            byte_start: self.byte_start + (self.text.len() - trimmed.len()),
        }
    }

    pub(crate) fn trim_ascii_end(self) -> Self {
        Self {
            text: self.text.trim_ascii_end(),
            byte_start: self.byte_start,
        }
    }

    pub(crate) fn trim_inline_ascii_start(self) -> Self {
        let trimmed = self.text.trim_start_matches([' ', '\t']);
        Self {
            text: trimmed,
            byte_start: self.byte_start + (self.text.len() - trimmed.len()),
        }
    }

    pub(crate) fn trim_inline_ascii_end(self) -> Self {
        Self {
            text: self.text.trim_end_matches([' ', '\t']),
            byte_start: self.byte_start,
        }
    }

    pub(crate) fn without_suffix(self, suffix: char) -> Option<Self> {
        Some(Self {
            text: self.text.strip_suffix(suffix)?,
            byte_start: self.byte_start,
        })
    }

    pub(crate) fn prefix(self, byte_length: usize) -> Option<Self> {
        Some(Self {
            text: self.text.get(..byte_length)?,
            byte_start: self.byte_start,
        })
    }

    pub(crate) fn suffix(self, relative_start: usize) -> Option<Self> {
        Some(Self {
            text: self.text.get(relative_start..)?,
            byte_start: self.byte_start.checked_add(relative_start)?,
        })
    }

    pub(crate) fn slice(self, relative_start: usize, relative_end: usize) -> Option<Self> {
        Some(Self {
            text: self.text.get(relative_start..relative_end)?,
            byte_start: self.byte_start.checked_add(relative_start)?,
        })
    }

    pub(crate) fn strip_prefix(self, prefix: &str) -> Option<Self> {
        self.suffix(prefix.len())
            .filter(|_| self.text.starts_with(prefix))
    }
}

pub(crate) struct SourceLines<'a> {
    input: &'a str,
    next_byte_start: usize,
    next_line_number: usize,
}

pub(crate) struct SourceCommands<'a> {
    lines: SourceLines<'a>,
    current_line: Option<SourceLine<'a>>,
    next_relative_start: usize,
}

impl<'a> SourceCommands<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            lines: SourceLines::new(input),
            current_line: None,
            next_relative_start: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Option<SourceCommand<'a>> {
        let line = match self.current_line {
            Some(line) => line,
            None => {
                let line = self.lines.next()?;
                self.current_line = Some(line);
                self.next_relative_start = 0;
                line
            }
        };
        let source = line.source();
        let command_start = self.next_relative_start;
        let mut in_tag = false;
        let mut in_arguments = false;
        let mut escaped = false;
        let mut has_non_space = false;

        for (relative_offset, byte) in source
            .text()
            .as_bytes()
            .get(command_start..)?
            .iter()
            .copied()
            .enumerate()
        {
            let index = command_start + relative_offset;
            if escaped {
                escaped = false;
                has_non_space = true;
                continue;
            }
            match byte {
                b'\\' if in_tag => {
                    escaped = true;
                    has_non_space = true;
                }
                b'[' if !in_tag && !in_arguments => {
                    in_tag = true;
                    has_non_space = true;
                }
                b']' if in_tag => {
                    in_tag = false;
                    has_non_space = true;
                }
                b'(' if !in_tag => {
                    in_arguments = true;
                    has_non_space = true;
                }
                b')' if in_arguments => {
                    in_arguments = false;
                    has_non_space = true;
                }
                b'#' if !in_tag && !in_arguments => {
                    let command = source.slice(command_start, index)?;
                    self.current_line = None;
                    return Some(SourceCommand {
                        line_number: line.line_number(),
                        source: command,
                        end_error_span: byte_span_from_valid_range(source.byte_start() + index, 1),
                    });
                }
                b'{' if !in_tag && !in_arguments => {
                    let command_end = index + 1;
                    let command = source.slice(command_start, command_end)?;
                    self.next_relative_start = command_end;
                    if command_end == source.text().len() {
                        self.current_line = None;
                    }
                    return Some(SourceCommand {
                        line_number: line.line_number(),
                        source: command,
                        end_error_span: byte_span_from_valid_range(source.byte_start() + index, 1),
                    });
                }
                b'}' if !in_tag && !in_arguments && !has_non_space => {
                    let command_end = index + 1;
                    let command = source.slice(command_start, command_end)?;
                    self.next_relative_start = command_end;
                    if command_end == source.text().len() {
                        self.current_line = None;
                    }
                    return Some(SourceCommand {
                        line_number: line.line_number(),
                        source: command,
                        end_error_span: byte_span_from_valid_range(source.byte_start() + index, 1),
                    });
                }
                byte if !matches!(byte, b' ' | b'\t' | b'\r') => {
                    has_non_space = true;
                }
                _ => {}
            }
        }

        let command = source.slice(command_start, source.text().len())?;
        self.current_line = None;
        Some(SourceCommand {
            line_number: line.line_number(),
            source: command,
            end_error_span: line.end_error_span(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceCommand<'a> {
    line_number: usize,
    source: SourceSlice<'a>,
    end_error_span: ByteSpan,
}

impl<'a> SourceCommand<'a> {
    pub(crate) const fn line_number(self) -> usize {
        self.line_number
    }

    pub(crate) const fn source(self) -> SourceSlice<'a> {
        self.source
    }

    pub(crate) const fn end_error_span(self) -> ByteSpan {
        self.end_error_span
    }
}

impl<'a> SourceLines<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input,
            next_byte_start: 0,
            next_line_number: 1,
        }
    }

    pub(crate) fn next(&mut self) -> Option<SourceLine<'a>> {
        if self.next_byte_start >= self.input.len() {
            return None;
        }
        let byte_start = self.next_byte_start;
        let bytes = self.input.as_bytes();
        let relative_newline = bytes
            .get(byte_start..)?
            .iter()
            .position(|byte| *byte == b'\n');
        let (text_end, terminator, next_byte_start) =
            if let Some(relative_newline) = relative_newline {
                let newline = byte_start + relative_newline;
                if newline > byte_start && bytes.get(newline - 1) == Some(&b'\r') {
                    (
                        newline - 1,
                        Some(SourceSlice::new(
                            self.input.get(newline - 1..newline + 1)?,
                            newline - 1,
                        )),
                        newline + 1,
                    )
                } else {
                    (
                        newline,
                        Some(SourceSlice::new(
                            self.input.get(newline..newline + 1)?,
                            newline,
                        )),
                        newline + 1,
                    )
                }
            } else {
                (self.input.len(), None, self.input.len())
            };
        let text = self.input.get(byte_start..text_end)?;
        self.next_byte_start = next_byte_start;
        let line_number = self.next_line_number;
        self.next_line_number = self.next_line_number.checked_add(1)?;
        Some(SourceLine {
            line_number,
            source: SourceSlice::new(text, byte_start),
            terminator,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceLine<'a> {
    line_number: usize,
    source: SourceSlice<'a>,
    terminator: Option<SourceSlice<'a>>,
}

impl<'a> SourceLine<'a> {
    pub(crate) const fn line_number(self) -> usize {
        self.line_number
    }

    pub(crate) const fn source(self) -> SourceSlice<'a> {
        self.source
    }

    #[cfg(test)]
    pub(crate) const fn terminator(self) -> Option<SourceSlice<'a>> {
        self.terminator
    }

    pub(crate) fn end_error_span(self) -> ByteSpan {
        self.terminator.map_or_else(
            || self.source.end_span(),
            |terminator| {
                terminator
                    .subspan(0, 1)
                    .unwrap_or_else(|| terminator.span())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "unit tests use fixed source-line fixtures with known lengths"
    )]

    use super::{SourceCommands, SourceLines, SourceSlice};

    #[test]
    fn source_lines_preserve_str_lines_text_and_exact_byte_offsets() {
        for input in [
            "",
            "\n",
            "\r\n",
            "a",
            "a\n",
            "a\r",
            "a\r\n",
            "a\n\n",
            "é\r\nb\n",
            "\r\né\r\n",
        ] {
            let expected = input.lines().collect::<Vec<_>>();
            let mut lines = SourceLines::new(input);
            let mut actual = Vec::new();
            let mut previous_end = 0;
            while let Some(line) = lines.next() {
                let source = line.source();
                let span = source.span();
                assert_eq!(
                    input.get(span.byte_start()..span.byte_end()),
                    Some(source.text()),
                    "{input:?}"
                );
                assert!(span.byte_start() >= previous_end, "{input:?}");
                previous_end = span.byte_end();
                actual.push(source.text());
            }
            assert_eq!(actual, expected, "{input:?}");
        }
    }

    #[test]
    fn source_lines_retain_lf_and_crlf_terminator_locations() {
        let mut lines = SourceLines::new("a\r\né\nz");
        let first = lines.next().expect("first line");
        assert_eq!(first.source().text(), "a");
        assert_eq!(first.terminator().map(SourceSlice::text), Some("\r\n"));
        assert_eq!(first.end_error_span().byte_start(), 1);
        assert_eq!(first.end_error_span().byte_length(), 1);

        let second = lines.next().expect("second line");
        assert_eq!(second.source().text(), "é");
        assert_eq!(second.terminator().map(SourceSlice::text), Some("\n"));
        assert_eq!(second.end_error_span().byte_start(), 5);

        let third = lines.next().expect("third line");
        assert_eq!(third.source().text(), "z");
        assert_eq!(third.terminator(), None);
        assert_eq!(third.end_error_span().byte_start(), 7);
        assert!(lines.next().is_none());
    }

    #[test]
    fn source_commands_split_inline_block_boundaries_without_splitting_tags_or_arguments() {
        let input = "REPEAT 2 { H[tag{value}](0.5) 0\n} M 0 # comment\n";
        let mut commands = SourceCommands::new(input);
        let actual = std::iter::from_fn(|| commands.next())
            .map(|command| {
                (
                    command.line_number(),
                    command.source().text().to_string(),
                    command.end_error_span(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual
                .iter()
                .map(|(_, text, _)| text.as_str())
                .collect::<Vec<_>>(),
            ["REPEAT 2 {", " H[tag{value}](0.5) 0", "}", " M 0 "]
        );
        assert_eq!(
            actual.iter().map(|(line, _, _)| *line).collect::<Vec<_>>(),
            [1, 1, 2, 2]
        );
        assert_eq!(
            actual.get(3).expect("fourth command").2.byte_start(),
            input.find('#').expect("comment")
        );
    }

    #[test]
    fn source_commands_yield_blank_and_comment_lines_for_line_limit_accounting() {
        let mut commands = SourceCommands::new("\n # comment\nH 0\n");
        let actual = std::iter::from_fn(|| commands.next())
            .map(|command| (command.line_number(), command.source().text().to_string()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                (1, String::new()),
                (2, " ".to_string()),
                (3, "H 0".to_string())
            ]
        );
    }
}
