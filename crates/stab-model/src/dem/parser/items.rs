use arrayvec::ArrayVec;

use super::super::{DemItem, DetectorErrorModel};

const INLINE_NESTED_DEM_ITEMS: usize = 2;

#[allow(
    clippy::large_enum_variant,
    reason = "two inline DEM items remove small-body heap over-allocation; parser nesting is capped at 256"
)]
pub(super) enum ParsedDemItems {
    Preallocated(Vec<DemItem>),
    Nested(ArrayVec<DemItem, INLINE_NESTED_DEM_ITEMS>),
}

impl ParsedDemItems {
    pub(super) fn top_level(capacity: usize) -> Self {
        Self::Preallocated(Vec::with_capacity(capacity))
    }

    pub(super) fn nested() -> Self {
        Self::Nested(ArrayVec::new())
    }

    pub(super) fn push(&mut self, item: DemItem) {
        match self {
            Self::Preallocated(items) => items.push(item),
            Self::Nested(items) if !items.is_full() => items.push(item),
            Self::Nested(items) => {
                let mut spilled = Vec::with_capacity(INLINE_NESTED_DEM_ITEMS * 2);
                spilled.extend(items.drain(..));
                spilled.push(item);
                *self = Self::Preallocated(spilled);
            }
        }
    }

    pub(super) fn into_model(self) -> DetectorErrorModel {
        let items = match self {
            Self::Preallocated(items) => items,
            Self::Nested(items) => {
                let mut exact = Vec::with_capacity(items.len());
                exact.extend(items);
                exact
            }
        };
        DetectorErrorModel::from_items(items)
    }
}
