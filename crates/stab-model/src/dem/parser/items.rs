use super::super::{DemItem, DetectorErrorModel};

const INITIAL_NESTED_DEM_CAPACITY: usize = 2;

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

pub(super) struct ParsedNestedDemItems {
    items: Vec<DemItem>,
}

impl ParsedNestedDemItems {
    pub(super) fn nested() -> Self {
        Self { items: Vec::new() }
    }
}

impl ParsedDemItemStorage for ParsedNestedDemItems {
    const STOPS_ON_TERMINATOR: bool = true;

    fn push_item(&mut self, item: DemItem) {
        if self.items.capacity() == 0 {
            self.items.reserve_exact(INITIAL_NESTED_DEM_CAPACITY);
        }
        self.items.push(item);
    }

    fn into_model(self) -> DetectorErrorModel {
        DetectorErrorModel::from_items(self.items)
    }
}
