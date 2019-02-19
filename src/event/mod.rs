//! Readiness event types and utilities.

use std::io;
use std::time::Duration;

mod events;
mod id;
mod ready;

pub use self::events::Events;
pub use self::id::EventedId;
pub use self::ready::Ready;

/// A readiness event.
///
/// `Event` is a [readiness state] paired with a [`EventedId`]. It is returned by
/// [`poll`].
///
/// For more documentation on polling and events, see [`poll`].
///
/// [readiness state]: Ready
/// [`poll`]: fn@crate::poll
///
/// # Examples
///
/// ```
/// use mio_st::event::{Event, EventedId, Ready};
///
/// let event = Event::new(EventedId(0), Ready::READABLE | Ready::WRITABLE);
///
/// assert_eq!(event.id(), EventedId(0));
/// assert_eq!(event.readiness(), Ready::READABLE | Ready::WRITABLE);
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Event {
    id: EventedId,
    readiness: Ready,
}

impl Event {
    /// Creates a new `Event` containing `id` and `readiness`.
    pub fn new(id: EventedId, readiness: Ready) -> Event {
        Event { id, readiness }
    }

    /// Returns the event's id.
    pub fn id(&self) -> EventedId {
        self.id
    }

    /// Returns the event's readiness.
    pub fn readiness(&self) -> Ready {
        self.readiness
    }
}

/// A readiness event source that can be polled for readiness events.
pub trait Source<Evts>
    where Evts: Events,
{
    /// The duration until the next event will be available.
    ///
    /// This is used to determine what timeout to use in a blocking call to
    /// poll. For example if we have a queue of timers, of which the next one
    /// expires in one second, we don't want to block for more then one second
    /// and thus we should return `Some(1 second)` to ensure that.
    ///
    /// If the duration until the next available event is unknown `None` should
    /// be returned.
    fn next_event_available(&self) -> Option<Duration>;

    /// Poll for events.
    ///
    /// Any available readiness events must be added to `events`. The pollable
    /// source may not block.
    ///
    /// Some implementation of `Events` have a limited available capacity.
    /// This method may not add more events then `Events::capacity_left`
    /// returns, if it returns a capacity limit.
    fn poll(&mut self, events: &mut Evts) -> io::Result<()>;
}

/// A blocking variant of [`Source`].
pub trait BlockingSource<Evts>: Source<Evts>
    where Evts: Events,
{
    /// A blocking poll for readiness events.
    ///
    /// This is the same as [`Source::poll`] and all requirements of that method
    /// apply to this method as well. Different to `poll` is that this method
    /// may block up `timeout` duration, if one is provided, or block forever if
    /// no timeout is provided (assuming *something* wakes up the poll source).
    fn blocking_poll(&mut self, events: &mut Evts, timeout: Option<Duration>) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use crate::event::{Event, EventedId, Ready};

    #[test]
    fn event() {
        let event = Event::new(EventedId(0), Ready::READABLE);
        assert_eq!(event.id(), EventedId(0));
        assert_eq!(event.readiness(), Ready::READABLE);
    }

    #[test]
    fn equality() {
        let event = Event::new(EventedId(0), Ready::WRITABLE);
        assert_eq!(event, event.clone());

        // Same
        let event2 = Event::new(EventedId(0), Ready::WRITABLE);
        assert_eq!(event, event2);

        // Different id.
        let event3 = Event::new(EventedId(1), Ready::WRITABLE);
        assert_ne!(event, event3);

        // Different readiness.
        let event4 = Event::new(EventedId(0), Ready::READABLE);
        assert_ne!(event, event4);
    }
}
