//! Module with user space readiness event queue.

use std::io;
use std::time::Duration;

use log::trace;

use crate::event::{self, Capacity, Event, Events, Ready};

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

    /// Notify an evented handle of an user space event.
    ///
    /// This uses the user space event system.
    ///
    pub fn notify(&mut self, id: event::Id, ready: Ready) {
        trace!("adding user space event: id={}, ready={:?}", id, ready);
        self.events.push_back(Event::new(id, ready));
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
