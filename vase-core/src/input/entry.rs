use std::time::{Duration, Instant};

/// How long a half-typed index waits for another digit before committing.
pub const ENTRY_TIMEOUT: Duration = Duration::from_millis(120);

/// The outcome of feeding a key or a clock tick to a `NumberEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Nothing was buffered and nothing was consumed.
    Idle,
    /// A digit was taken; more may follow.
    Pending,
    /// Act on this 1-based number.
    Commit(usize),
}

/// A number typed one digit at a time, committing as soon as no larger valid number could follow and otherwise after `ENTRY_TIMEOUT`, so single-digit picks stay instant in a long list.
#[derive(Debug, Default)]
pub struct NumberEntry {
    pending: Option<usize>,
    deadline: Option<Instant>,
}

impl NumberEntry {
    /// Take one digit, where `max` is the largest number currently valid. An out-of-range digit is dropped and whatever was buffered commits.
    pub fn digit(&mut self, d: usize, max: usize, now: Instant) -> Entry {
        let new = self.pending.unwrap_or(0) * 10 + d;
        if new == 0 || new > max {
            return self.flush();
        }
        self.pending = Some(new);
        self.deadline = Some(now + ENTRY_TIMEOUT);
        if new * 10 > max {
            return self.flush(); // no larger number could follow, act now
        }
        Entry::Pending
    }

    /// Commit whatever is buffered, e.g. because a non-digit key ended the entry.
    pub fn flush(&mut self) -> Entry {
        self.deadline = None;
        match self.pending.take() {
            Some(n) => Entry::Commit(n),
            None => Entry::Idle,
        }
    }

    /// Discard whatever is buffered without acting on it.
    pub fn cancel(&mut self) {
        self.pending = None;
        self.deadline = None;
    }

    /// Commit a half-typed number once its deadline has passed.
    pub fn tick(&mut self, now: Instant) -> Entry {
        match self.deadline {
            Some(d) if now >= d => self.flush(),
            _ => Entry::Idle,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}
