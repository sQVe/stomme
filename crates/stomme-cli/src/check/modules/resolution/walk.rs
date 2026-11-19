use std::collections::BTreeMap;

use super::Module;

pub(super) type Pair = (Vec<String>, String);

const LOOKUP_BUDGET: usize = 10_000;

/// State of one `reached_files` call.
pub(super) struct Walk<'a> {
    pub(super) module: &'a Module,
    /// Open pairs, each with its index on the lookup stack.
    open: BTreeMap<Pair, usize>,
    /// Lowest stack index that a lookup cut since its enclosing lookup began.
    lowest_cut: Option<usize>,
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

    pub(super) fn cut_if_open(&mut self, pair: &Pair) -> bool {
        let Some(&index) = self.open.get(pair) else {
            return false;
        };

        self.record_cut(index);

        true
    }

    pub(super) fn enter(&mut self, pair: Pair) -> OpenLookup {
        let index = self.open.len();
        let outer_cut = self.lowest_cut.take();
        self.open.insert(pair.clone(), index);

        OpenLookup {
            pair,
            index,
            outer_cut,
        }
    }

    pub(super) fn leave(&mut self, lookup: OpenLookup) -> bool {
        self.open.remove(&lookup.pair);
        let open_cut = self.lowest_cut.filter(|cut| *cut < lookup.index);
        self.lowest_cut = [lookup.outer_cut, open_cut].into_iter().flatten().min();

        open_cut.is_none()
    }

    pub(super) fn begin_nested(&mut self) -> Option<usize> {
        self.lowest_cut.take()
    }

    pub(super) fn end_nested(&mut self, outer_cut: Option<usize>) -> bool {
        let nested_cut = self.lowest_cut;
        self.lowest_cut = [outer_cut, nested_cut].into_iter().flatten().min();

        nested_cut.is_none()
    }

    fn record_cut(&mut self, index: usize) {
        self.lowest_cut = Some(self.lowest_cut.map_or(index, |lowest| lowest.min(index)));
    }
}

pub(super) struct OpenLookup {
    pair: Pair,
    index: usize,
    outer_cut: Option<usize>,
}
