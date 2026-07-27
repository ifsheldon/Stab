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

    /// Number of records that must be encoded as one complete group.
    pub const fn records_per_group(self) -> usize {
        match self {
            Self::Ptb64 => 64,
            Self::ZeroOne | Self::B8 | Self::R8 | Self::Hits | Self::Dets => 1,
        }
    }
}

/// One result codec registration exposed through [`crate::CapabilitySet`].
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

pub(crate) const fn codec_capabilities() -> &'static [CodecCapability; 6] {
    &CODECS
}
