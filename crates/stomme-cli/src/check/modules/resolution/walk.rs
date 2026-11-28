use std::collections::BTreeMap;

use super::Module;

pub(super) type Pair = (Vec<String>, String);

const LOOKUP_BUDGET: usize = 10_000;
pub(super) const MAX_DEPTH: usize = 256;

/// State of one `reached_files` call.
pub(super) struct Walk<'a> {
    pub(super) module: &'a Module,
    /// Open pairs, each with its index on the lookup stack.
    open: BTreeMap<Pair, usize>,
    /// Lowest stack index that a lookup cut since its enclosing lookup began.
    lowest_cut: Option<usize>,
    remaining: usize,
    pub(super) exhausted: bool,
    pub(super) depth_exceeded: bool,
}

impl<'a> Walk<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        Self {
            module,
            open: BTreeMap::new(),
            lowest_cut: None,
            remaining: LOOKUP_BUDGET,
            exhausted: false,
            depth_exceeded: false,
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

    // Stops the walk before a lookup opens more than `MAX_DEPTH` frames.
    pub(super) fn can_look_up(&mut self) -> bool {
        if self.depth_exceeded {
            return false;
        }

        if self.open.len() < MAX_DEPTH {
            return true;
        }

        self.depth_exceeded = true;

        false
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

    pub(super) fn leave(&mut self, lookup: OpenLookup, child_module_returned: bool) -> bool {
        self.open.remove(&lookup.pair);
        let cut_below = self.lowest_cut.filter(|cut| *cut < lookup.index);
        let self_cut = self.lowest_cut == Some(lookup.index);
        self.lowest_cut = [lookup.outer_cut, cut_below].into_iter().flatten().min();

        // A cut at this lookup's own index returns this lookup's children. A returned module
        // makes the result depend on where the cycle was entered, so it is not cacheable. An
        // empty return adds no module to the result, and vetoing it makes a fan-in alias chain
        // exponential.
        let self_cut_returns_module = self_cut && child_module_returned;

        cut_below.is_none() && !self_cut_returns_module
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
