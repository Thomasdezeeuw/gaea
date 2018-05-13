/// Associates readiness notifications with [`Evented`] handles.
///
/// `EventedId` is used as an argument to [`Poll.register`] and
/// [`Poll.reregister`] and is used to associate an [`Event`] with an
/// [`Evented`] handle.
///
/// See [`Poll`] for more documentation on polling.
///
/// # Uniqueness of `EventedId`
///
/// `EventedId` does not have to be unique within a `Poll` instance, it is
/// purely a tool for the user of `Poll` to associate an `Event` with an
/// `Evented` handle. It is advised for example to use the same `EventedId` for
/// say a `TcpStream` and any related timeout or deadline for the same
/// connection. The `EventedID` is effectively opaque to `Poll`, as long as it
/// is valid.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poll.register`]: ../struct.Poll.html#method.register
/// [`Poll.reregister`]: ../struct.Poll.html#method.reregister
/// [`Event`]: ../event/struct.Event.html
/// [`Poll`]: ../struct.Poll.html
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventedId(pub usize);

/// The only invalid evented id.
///
/// [`EventedId.is_valid`] can be used to determine if the id is valid.
pub(crate) const INVALID_EVENTED_ID: EventedId = EventedId(::std::usize::MAX);

impl EventedId {
    /// Whether or not the `EventedId` is valid.
    pub fn is_valid(&self) -> bool {
        *self != INVALID_EVENTED_ID
    }
}

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
