//! Module with user space readiness event queue.

use std::time::Duration;

use log::trace;

use crate::event::{self, Capacity, Event, Events};

/// User space readiness queue.
///
/// A simple, single threaded user space readiness event queue. This implements
/// [`event::Source`] which can be used to poll for readiness events.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use mio_st::event::Source;
/// use mio_st::{Event, Queue, Ready, event};
///
/// let mut queue = Queue::new();
/// let mut events = Vec::new();
///
/// // Add a new event.
/// let event = Event::new(event::Id(0), Ready::READABLE);
/// queue.add(event);
///
/// // Poll the queue.
/// Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
/// assert_eq!(events.get(0), Some(&event));
/// #     Ok(())
/// # }
/// ```
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

impl<Evts, E> event::Source<Evts, E> for Queue
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        if !self.events.is_empty() {
            Some(Duration::from_millis(0))
        } else {
            None
        }
    }

    fn poll(&mut self, events: &mut Evts) -> Result<(), E> {
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
