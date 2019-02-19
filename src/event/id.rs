use std::fmt;

/// Identifier of an event.
///
/// This is used to associate a readiness notifications with event handle.
///
/// See [`poll`] for more documentation on polling.
///
/// [`poll`]: fn@crate::poll
///
/// # Uniqueness of `EventedId`
///
/// `EventedId` does not have to be unique, it is purely a tool for the user to
/// associate an `Event` with an event handle. It is advised for example to use
/// the same `EventedId` for say a `TcpStream` and any related timeout or
/// deadline for the same connection. The `EventedID` is effectively opaque to
/// any readiness event sources.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct EventedId(pub usize);

impl From<usize> for EventedId {
    fn from(val: usize) -> EventedId {
        EventedId(val)
    }
}

impl From<EventedId> for usize {
    fn from(val: EventedId) -> usize {
        val.0
    }
}

impl fmt::Display for EventedId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use crate::event::EventedId;

    #[test]
    fn event() {
        let id = EventedId(0);
        assert_eq!(EventedId::from(0), EventedId(0));
        assert_eq!(usize::from(id), 0);
        assert_eq!(id.0, 0);
        assert_eq!(id, id.clone());

        let max_value = usize::max_value();
        let id = EventedId(max_value);
        assert_eq!(EventedId::from(max_value), EventedId(max_value));
        assert_eq!(usize::from(id), max_value);
        assert_eq!(id.0, max_value);
        assert_eq!(id, id.clone());
    }
}
