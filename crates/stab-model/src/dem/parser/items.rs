use super::super::{DemItem, DemTag, DetectorErrorModel};
use crate::{ByteSpan, DemRepeatCount};

const INITIAL_NESTED_DEM_CAPACITY: usize = 2;

/// Items of one block being parsed.
///
/// The top-level block grows like an ordinary vector, while a repeat body starts from a small
/// exact reservation because most bodies hold only a few items.
pub(super) struct ParsedDemItems {
    items: Vec<DemItem>,
    nested: bool,
}

impl ParsedDemItems {
    pub(super) const fn top_level() -> Self {
        Self {
            items: Vec::new(),
            nested: false,
        }
    }

    pub(super) const fn nested() -> Self {
        Self {
            items: Vec::new(),
            nested: true,
        }
    }

    pub(super) fn push_item(&mut self, item: DemItem) {
        if self.nested {
            self.push_nested(item);
        } else {
            self.items.push(item);
        }
    }

    // Kept out of line so the top-level push stays a plain inline vector push.
    #[inline(never)]
    fn push_nested(&mut self, item: DemItem) {
        if self.items.capacity() == 0 {
            self.items.reserve_exact(INITIAL_NESTED_DEM_CAPACITY);
        }
        self.items.push(item);
    }

    pub(super) fn into_model(self) -> DetectorErrorModel {
        DetectorErrorModel::from_items(self.items)
    }
}

/// A `repeat` block whose body is being parsed.
///
/// The parser keeps one frame per open block instead of recursing, and admits the repeat
/// nesting before it pushes a frame, so the frame stack never exceeds the parse limits'
/// repeat-nesting limit.
pub(super) struct OpenDemRepeat {
    /// Items of the enclosing block parsed so far.
    pub(super) parent: ParsedDemItems,
    pub(super) header: OpenDemRepeatHeader,
}

/// The parsed header of an open `repeat` block.
pub(super) struct OpenDemRepeatHeader {
    pub(super) count: DemRepeatCount,
    pub(super) tag: Option<DemTag>,
    /// Header line and span, which the tag charge reports when the block closes.
    pub(super) line_number: usize,
    pub(super) span: ByteSpan,
}
