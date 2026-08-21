use arrayvec::ArrayVec;

use super::super::{DemItem, DetectorErrorModel};

const INLINE_NESTED_DEM_ITEMS: usize = 2;

pub(super) trait ParsedDemItemStorage {
    const STOPS_ON_TERMINATOR: bool;

    fn push_item(&mut self, item: DemItem);

    fn into_model(self) -> DetectorErrorModel;
}

impl ParsedDemItemStorage for Vec<DemItem> {
    const STOPS_ON_TERMINATOR: bool = false;

    fn push_item(&mut self, item: DemItem) {
        self.push(item);
    }

    fn into_model(self) -> DetectorErrorModel {
        DetectorErrorModel::from_items(self)
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "two inline DEM items remove small-body heap over-allocation; parser nesting is capped at 256"
)]
pub(super) enum ParsedNestedDemItems {
    Inline(ArrayVec<DemItem, INLINE_NESTED_DEM_ITEMS>),
    Spilled(Vec<DemItem>),
}

impl ParsedNestedDemItems {
    pub(super) fn nested() -> Self {
        Self::Inline(ArrayVec::new())
    }
}

impl ParsedDemItemStorage for ParsedNestedDemItems {
    const STOPS_ON_TERMINATOR: bool = true;

    fn push_item(&mut self, item: DemItem) {
        match self {
            Self::Spilled(items) => items.push(item),
            Self::Inline(items) if !items.is_full() => items.push(item),
            Self::Inline(items) => {
                let mut spilled = Vec::with_capacity(INLINE_NESTED_DEM_ITEMS * 2);
                spilled.extend(items.drain(..));
                spilled.push(item);
                *self = Self::Spilled(spilled);
            }
        }
    }

    fn into_model(self) -> DetectorErrorModel {
        let items = match self {
            Self::Spilled(items) => items,
            Self::Inline(items) => {
                let mut exact = Vec::with_capacity(items.len());
                exact.extend(items);
                exact
            }
        };
        DetectorErrorModel::from_items(items)
    }
}
