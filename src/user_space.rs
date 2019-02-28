//! Module with user space readiness event queue.

use std::io;
use std::time::Duration;

use log::trace;

use crate::event::{self, Capacity, Event, Events};

#[derive(Debug)]
pub struct Queue {
    events: Vec<Event>,
}

impl Queue {
    /// Create a new user space readiness event queue.
    pub fn new() -> Queue {
        Queue {
            events: Vec::new(),
        }
    }

    /// Add a new readiness event.
    pub fn add(&mut self, event: Event) {
        trace!("adding user space event: id={}, readiness={:?}",
            event.id(), event.readiness());
        self.events.push(event);
    }
}

impl<Evts> event::Source<Evts> for Queue
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        if !self.events.is_empty() {
            Some(Duration::from_millis(0))
        } else {
            None
        }
    }

    fn poll(&mut self, events: &mut Evts) -> io::Result<()> {
        trace!("polling user space events");
        let drain = if let Capacity::Limited(capacity_left) = events.capacity_left() {
            self.events.drain(..capacity_left)
        } else {
            self.events.drain(..)
        };
        events.extend(drain);
        Ok(())
    }
}
