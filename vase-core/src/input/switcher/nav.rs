use super::{Item, Pick, Switcher};

impl<T: Item> Switcher<T> {
    pub(super) fn matching(&self) -> impl Iterator<Item = &(T, String)> {
        let q = self.query.to_lowercase();
        self.items.iter().filter(move |(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
    }

    pub(super) fn visible_len(&self) -> usize {
        self.matching().count()
    }

    /// Number of rows a digit can address.
    pub(super) fn numbered_len(&self) -> usize {
        self.matching().filter(|(it, _)| it.numbered()).count()
    }

    pub(super) fn take_selected(&mut self) -> Pick<T> {
        match self.matching().nth(self.selected) {
            Some((it, _)) => Pick::Chosen(it.clone()),
            None => Pick::Cancelled,
        }
    }

    /// Choose the `n`-th (1-based) numbered row.
    pub(super) fn take_numbered(&mut self, n: usize) -> Pick<T> {
        match self.matching().filter(|(it, _)| it.numbered()).nth(n - 1) {
            Some((it, _)) => Pick::Chosen(it.clone()),
            None => Pick::Ignored,
        }
    }

    /// Move one row in `delta`'s direction (wrapping), then settle onto a selectable row.
    pub(super) fn step(&mut self, delta: isize) {
        let n = self.visible_len();
        if n == 0 {
            return;
        }
        self.selected = if delta < 0 {
            if self.selected == 0 {
                n - 1
            } else {
                self.selected - 1
            }
        } else if self.selected + 1 >= n {
            0
        } else {
            self.selected + 1
        };
        self.settle(delta);
    }

    /// Advance the selection past unselectable rows so the cursor never rests on one.
    pub(super) fn settle(&mut self, delta: isize) {
        let n = self.visible_len();
        for _ in 0..n {
            let on_row = self.matching().nth(self.selected).map(|(it, _)| it.selectable()).unwrap_or(true);
            if on_row {
                return;
            }
            self.selected = if delta < 0 {
                if self.selected == 0 {
                    n - 1
                } else {
                    self.selected - 1
                }
            } else if self.selected + 1 >= n {
                0
            } else {
                self.selected + 1
            };
        }
    }

    pub(super) fn clamp(&mut self) {
        self.selected = self.selected.min(self.visible_len().saturating_sub(1));
    }
}
