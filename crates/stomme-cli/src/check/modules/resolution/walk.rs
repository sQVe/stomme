use std::collections::BTreeMap;

use super::Module;

type Pair = (Vec<String>, String);

const LOOKUP_BUDGET: usize = 10_000;

/// State of one `reached_files` call.
pub(super) struct Walk<'a> {
    pub(super) module: &'a Module,
    /// Open pairs, each with its index on the lookup stack.
    pub(super) open: BTreeMap<Pair, usize>,
    /// Lowest stack index that a lookup cut since its enclosing lookup began.
    pub(super) lowest_cut: Option<usize>,
    remaining: usize,
    pub(super) exhausted: bool,
}

impl<'a> Walk<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        Self {
            module,
            open: BTreeMap::new(),
            lowest_cut: None,
            remaining: LOOKUP_BUDGET,
            exhausted: false,
        }
    }

    pub(super) fn spend(&mut self) -> bool {
        if self.remaining == 0 {
            self.exhausted = true;

            return false;
        }

        self.remaining -= 1;

        true
    }

    pub(super) fn record_cut(&mut self, index: usize) {
        self.lowest_cut = Some(self.lowest_cut.map_or(index, |lowest| lowest.min(index)));
    }
}
