//! Module with timers.

use std::io;
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::time::{Duration, Instant};
use std::mem::replace;

use log::trace;

use crate::event::{self, Event, Events, Ready};

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

impl<Evts> event::Source<Evts> for Timers
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

    fn poll(&mut self, events: &mut Evts) -> io::Result<()> {
        trace!("polling timers");
        let now = Instant::now();

        for _ in 0..events.capacity_left().min(usize::max_value()) {
            match self.deadlines.peek() {
                Some(deadline) if deadline.0.deadline <= now => {
                    let deadline = self.deadlines.pop().unwrap().0;
                    events.push(Event::new(deadline.id, Ready::TIMER));
                },
                _ => break,
            }
        }
        Ok(())
    }
}
