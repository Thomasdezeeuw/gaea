//! Module with timers.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::mem::replace;
use std::time::{Duration, Instant};

use log::trace;

use crate::event::{self, Event, Events, Ready};

/// Timer readiness queue.
///
/// Polling this event source never returns an error.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::time::Instant;
///
/// use mio_st::{Event, Timers, Ready, event, poll};
///
/// let mut timers = Timers::new();
/// let mut events = Vec::new();
///
/// // Add a deadline, to trigger an event immediately.
/// let id = event::Id(0);
/// timers.add_deadline(id, Instant::now());
///
/// // Now we poll for events. Note that this is safe to unwrap as polling
/// // `Timers` never returns an error.
/// poll::<_, ()>(&mut [&mut timers], &mut events, None).unwrap();
///
/// assert_eq!(events.get(0), Some(&Event::new(id, Ready::TIMER)));
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Timers {
    deadlines: BinaryHeap<Reverse<Deadline>>,
}

/// A deadline.
///
/// This must be ordered by `deadline`, then `id`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Deadline {
    deadline: Instant,
    id: event::Id,
}

impl Timers {
    /// Create a new time event source.
    pub fn new() -> Timers {
        Timers {
            deadlines: BinaryHeap::new(),
        }
    }

    /// Add a new deadline.
    ///
    /// This will cause an event to trigger after the `deadline` has passed with
    /// the [`Ready::TIMER`] readiness and provided `id`.
    pub fn add_deadline(&mut self, id: event::Id, deadline: Instant) {
        trace!("adding deadline: id={}, deadline={:?}", id, deadline);
        self.deadlines.push(Reverse(Deadline { id, deadline }));
    }

    /// Add a new timeout.
    ///
    /// This is the same as [`add_deadline`], but then using a `Duration`, see
    /// [`add_deadline`] for more information.
    ///
    /// [`add_deadline`]: `Timers::add_deadline`
    pub fn add_timeout(&mut self, id: event::Id, timeout: Duration) {
        self.add_deadline(id, Instant::now() + timeout);
    }

    /// Remove a previously added deadline.
    ///
    /// # Notes
    ///
    /// Removing a deadline is a costly operation. For better performance it is
    /// advised to not bother with removing and instead ignore the event when it
    /// comes up.
    pub fn remove_deadline(&mut self, id: event::Id) {
        trace!("removing deadline: id={}", id);

        // TODO: optimize this.
        let index = self.deadlines.iter()
            .position(|deadline| deadline.0.id == id);

        if let Some(index) = index {
            let deadlines = replace(&mut self.deadlines, BinaryHeap::new());
            let mut deadlines_vec = deadlines.into_vec();
            let removed_deadline = deadlines_vec.swap_remove(index);
            debug_assert_eq!(removed_deadline.0.id, id, "remove_deadline: removed incorrect deadline");
            drop(replace(&mut self.deadlines, BinaryHeap::from(deadlines_vec)));
        }
    }
}

impl<Evts, E> event::Source<Evts, E> for Timers
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        self.deadlines.peek().map(|deadline| {
            let now = Instant::now();
            if deadline.0.deadline <= now {
                // Deadline has already expired, so no blocking.
                Duration::from_millis(0)
            } else {
                // Time between the deadline and right now.
                deadline.0.deadline.duration_since(now)
            }
        })
    }

    fn poll(&mut self, events: &mut Evts) -> Result<(), E> {
        trace!("polling timers");
        let now = Instant::now();

        for _ in 0..events.capacity_left().min(self.deadlines.len()) {
            match self.deadlines.peek() {
                Some(deadline) if deadline.0.deadline <= now => {
                    let deadline = self.deadlines.pop().unwrap().0;
                    events.add(Event::new(deadline.id, Ready::TIMER));
                },
                _ => break,
            }
        }
        Ok(())
    }
}
