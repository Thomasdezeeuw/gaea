//! Readiness event types and utilities.

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
