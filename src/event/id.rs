use std::fmt;

/// Associates readiness notifications with [`Evented`] handles.
///
/// `EventedId` is used as an argument to [`Poller.register`] and
/// [`Poller.reregister`] and is used to associate an [`Event`] with an
/// [`Evented`] handle.
///
/// See [`Poller`] for more documentation on polling.
///
/// # Uniqueness of `EventedId`
///
/// `EventedId` does not have to be unique within a `Poller` instance, it is
/// purely a tool for the user of `Poller` to associate an `Event` with an
/// `Evented` handle. It is advised for example to use the same `EventedId` for
/// say a `TcpStream` and any related timeout or deadline for the same
/// connection. The `EventedID` is effectively opaque to `Poller`, as long as it
/// is valid.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poller.register`]: ../poll/struct.Poller.html#method.register
/// [`Poller.reregister`]: ../poll/struct.Poller.html#method.reregister
/// [`Event`]: ../event/struct.Event.html
/// [`Poller`]: ../poll/struct.Poller.html
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
