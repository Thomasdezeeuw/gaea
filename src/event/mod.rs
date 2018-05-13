//! Readiness event types and utilities.

use poll::Ready;

mod evented;
mod events;
mod id;

pub use self::evented::Evented;
pub use self::events::Events;
pub use self::id::EventedId;

pub(crate) use self::id::INVALID_EVENTED_ID;

/// A readiness event.
///
/// `Event` is a [readiness state] paired with a [`EventedId`]. It is returned by
/// [`Poll.poll`].
///
/// For more documentation on polling and events, see [`Poll`].
///
/// # Examples
///
/// ```
/// use mio_st::poll::Ready;
/// use mio_st::event::{Event, EventedId};
///
/// let event = Event::new(EventedId(0), Ready::READABLE | Ready::WRITABLE);
///
/// assert_eq!(event.id(), EventedId(0));
/// assert_eq!(event.readiness(), Ready::READABLE | Ready::WRITABLE);
/// ```
///
/// [readiness state]: ../struct.Ready.html
/// [`EventedId`]: struct.EventedId.html
/// [`Poll.poll`]: ../struct.Poll.html#method.poll
/// [`Poll`]: ../struct.Poll.html
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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
