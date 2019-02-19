//! Module with user space readiness event queue.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use log::trace;

use crate::event::{Event, Events, EventedId, Ready};
use crate::poll::Poll;

#[derive(Debug)]
pub struct Queue {
    events: VecDeque<Event>,
}

impl Queue {
    /// Create a new user space readiness event queue.
    pub fn new() -> Queue {
        Queue {
            events: VecDeque::new(),
        }
    }

    /// Notify an evented handle of an user space event.
    ///
    /// This uses the user space event system.
    ///
    pub fn notify(&mut self, id: EventedId, ready: Ready) {
        trace!("adding user space event: id={}, ready={:?}", id, ready);
        self.events.push_back(Event::new(id, ready));
    }
}

impl<Evts> Poll<Evts> for Queue
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
        let drain = if let Some(capacity_left) = events.capacity_left() {
            self.events.drain(..capacity_left)
        } else {
            self.events.drain(..)
        };
        events.extend(drain);
        Ok(())
    }
}
