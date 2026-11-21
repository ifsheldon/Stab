/// A half-open byte range in the original input.
///
/// Every span satisfies `byte_start + byte_length <= usize::MAX`, so [`Self::byte_end`] never
/// overflows.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByteSpan {
    byte_start: usize,
    byte_length: usize,
}

impl ByteSpan {
    /// The span of `byte_length` bytes starting at `byte_start`, or `None` when its end would
    /// overflow.
    pub const fn try_new(byte_start: usize, byte_length: usize) -> Option<Self> {
        match byte_start.checked_add(byte_length) {
            Some(_) => Some(Self {
                byte_start,
                byte_length,
            }),
            None => None,
        }
    }

    /// The empty span at `byte_start`.
    pub const fn empty_at(byte_start: usize) -> Self {
        Self {
            byte_start,
            byte_length: 0,
        }
    }

    pub const fn byte_start(self) -> usize {
        self.byte_start
    }

    pub const fn byte_length(self) -> usize {
        self.byte_length
    }

    pub const fn byte_end(self) -> usize {
        self.byte_start + self.byte_length
    }
}

/// Machine-readable diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ByteSpan;

    #[test]
    fn spans_whose_end_overflows_cannot_be_constructed() {
        assert_eq!(ByteSpan::try_new(usize::MAX, 1), None);
        let span = ByteSpan::try_new(usize::MAX - 3, 3).map(ByteSpan::byte_end);
        assert_eq!(span, Some(usize::MAX));
        assert_eq!(ByteSpan::empty_at(usize::MAX).byte_end(), usize::MAX);
    }
}
