//! A prepared implementation and its immutable selection receipt.

use crate::compute::registry::support::StrategySelection;

pub(crate) struct Prepared<S> {
    pub(crate) implementation: S,
    selection: StrategySelection,
}

impl<S> Prepared<S> {
    pub(crate) fn new(implementation: S, selection: StrategySelection) -> Self {
        Self { implementation, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }
}
