use crate::EncodedSizeEstimate as Estimate;
use crate::SampleFormat;

/// Physical encoding used by a registered result-record codec.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordEncoding {
    Text,
    BytePacked,
    RunLength,
    BitPlane64,
}

impl RecordEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::BytePacked => "byte-packed",
            Self::RunLength => "run-length",
            Self::BitPlane64 => "bit-plane-64",
        }
    }
}

/// Result-record format registered by Stab's codec layer.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}

impl RecordFormat {
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        CODECS.iter().map(|codec| codec.format())
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZeroOne => "01",
            Self::B8 => "b8",
            Self::R8 => "r8",
            Self::Hits => "hits",
            Self::Dets => "dets",
            Self::Ptb64 => "ptb64",
        }
    }

    pub const fn encoding(self) -> RecordEncoding {
        match self {
            Self::ZeroOne | Self::Hits | Self::Dets => RecordEncoding::Text,
            Self::B8 => RecordEncoding::BytePacked,
            Self::R8 => RecordEncoding::RunLength,
            Self::Ptb64 => RecordEncoding::BitPlane64,
        }
    }

    /// Per-record sample encoding driven through [`crate::MeasureRecordWriter`].
    ///
    /// PTB64 is the one registered exception: it interleaves complete 64-record groups into bit
    /// planes, so it has no per-record writer encoding and returns `None`.
    pub const fn sample_format(self) -> Option<SampleFormat> {
        match self {
            Self::ZeroOne => Some(SampleFormat::ZeroOne),
            Self::B8 => Some(SampleFormat::B8),
            Self::R8 => Some(SampleFormat::R8),
            Self::Hits => Some(SampleFormat::Hits),
            Self::Dets => Some(SampleFormat::Dets),
            Self::Ptb64 => None,
        }
    }

    /// Number of records that must be encoded as one complete group.
    pub const fn records_per_group(self) -> usize {
        match self {
            Self::Ptb64 => 64,
            Self::ZeroOne | Self::B8 | Self::R8 | Self::Hits | Self::Dets => 1,
        }
    }

    /// Estimates encoded bytes for fixed-width records without producing any records.
    ///
    /// Sparse encodings depend on record contents and therefore return [`Estimate::Unknown`].
    /// PTB64 is exact only for a complete number of 64-record groups.
    pub fn estimate_output_bytes(
        self,
        record_count: usize,
        bits_per_record: usize,
    ) -> Estimate<usize> {
        let bytes = match self {
            Self::ZeroOne => bits_per_record
                .checked_add(1)
                .and_then(|per_record| record_count.checked_mul(per_record)),
            Self::B8 => record_count.checked_mul(bits_per_record.div_ceil(8)),
            Self::Ptb64 if record_count.is_multiple_of(64) => record_count
                .checked_div(64)
                .and_then(|groups| groups.checked_mul(bits_per_record))
                .and_then(|words| words.checked_mul(size_of::<u64>())),
            Self::R8 | Self::Hits | Self::Dets | Self::Ptb64 => None,
        };
        bytes.map_or(Estimate::Unknown, Estimate::Exact)
    }
}

/// One result codec registration exposed through the codec registry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CodecCapability {
    format: RecordFormat,
    can_decode: bool,
    can_encode: bool,
    requires_typed_layout: bool,
}

impl CodecCapability {
    const fn new(format: RecordFormat) -> Self {
        Self {
            format,
            can_decode: true,
            can_encode: true,
            requires_typed_layout: matches!(format, RecordFormat::Dets),
        }
    }

    pub const fn format(self) -> RecordFormat {
        self.format
    }

    pub const fn can_decode(self) -> bool {
        self.can_decode
    }

    pub const fn can_encode(self) -> bool {
        self.can_encode
    }

    /// Whether namespace counts, rather than one untyped record width, define the layout.
    pub const fn requires_typed_layout(self) -> bool {
        self.requires_typed_layout
    }
}

const CODECS: [CodecCapability; 6] = [
    CodecCapability::new(RecordFormat::ZeroOne),
    CodecCapability::new(RecordFormat::B8),
    CodecCapability::new(RecordFormat::R8),
    CodecCapability::new(RecordFormat::Hits),
    CodecCapability::new(RecordFormat::Dets),
    CodecCapability::new(RecordFormat::Ptb64),
];

pub const fn codec_capabilities() -> &'static [CodecCapability; 6] {
    &CODECS
}

#[cfg(test)]
mod tests {
    use super::RecordFormat;
    use crate::SampleFormat;

    #[test]
    fn sample_format_owns_the_ptb64_per_record_exception() {
        assert_eq!(
            RecordFormat::ZeroOne.sample_format(),
            Some(SampleFormat::ZeroOne)
        );
        assert_eq!(RecordFormat::B8.sample_format(), Some(SampleFormat::B8));
        assert_eq!(RecordFormat::R8.sample_format(), Some(SampleFormat::R8));
        assert_eq!(RecordFormat::Hits.sample_format(), Some(SampleFormat::Hits));
        assert_eq!(RecordFormat::Dets.sample_format(), Some(SampleFormat::Dets));
        assert_eq!(RecordFormat::Ptb64.sample_format(), None);
        let group_only = RecordFormat::all()
            .filter(|format| format.sample_format().is_none())
            .count();
        assert_eq!(
            group_only, 1,
            "ptb64 is the one registered format without a per-record sample encoding"
        );
    }
}
