//! Readiness event types and utilities.

use std::io;

use sys;
use poll::{Poll, PollOpt, Ready, Private};

/// A value that may be registered with `Poll`.
///
/// Values that implement `Evented` can be registered with [`Poll`]. The methods
/// on the trait cannot be called directly, instead the equivalent methods must
/// be called on a [`Poll`] instance.
///
/// See [`Poll`] for more details.
///
/// # Implementing `Evented`
///
/// There are two types of `Evented` values.
///
/// * **System** handles, which are backed by sockets or other system handles.
///   These `Evented` handles will be monitored by the system selector. In this
///   case, an implementation of `Evented` delegates to a lower level handle.
///   Examples of this are [`TcpStream`]s, or the *unix only* [`EventedFd`].
///
/// * **User** handles, which are driven entirely in user space using
///   [`Registration`] and [`Notifier`]. In this case, the implementer takes
///   responsibility for driving the readiness state changes.
///
/// [`Poll`]: ../poll/struct.Poll.html
/// [`Registration`]: ../registration/struct.Registration.html
/// [`Notifier`]: ../registration/struct.Notifier.html
/// [`TcpStream`]: ../net/struct.TcpStream.html
/// [`EventedFd`]: ../unix/struct.EventedFd.html
///
/// # Dropping `Evented` types
///
/// All `Evented` types, unless otherwise specified, need to be deregistered
/// before being dropped for them to not leak resources. This goes against the
/// normal drop behaviour of types in Rust which cleanup after themselves, e.g.
/// a `File` will close itself. However since deregistering needs mutable access
/// to `Poll` this cannot be done while being dropped.
///
/// # Examples
///
/// Implementing `Evented` on a struct containing a system handle, such as a
/// [`TcpStream`].
///
/// ```
/// use std::io;
///
/// use mio_st::event::{Evented, EventedId};
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poll, PollOpt, Ready, Private};
///
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
///         // Delegate the `register` call to `socket`
///         self.socket.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`
///         self.socket.reregister(poll, id, interests, opt, p)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
///         // Delegate the `deregister` call to `socket`
///         self.socket.deregister(poll, p)
///     }
/// }
/// ```
///
/// Implementing `Evented` using a user space handle, using [`Registration`] and
/// [`Notifier`],
///
/// ```
/// use std::io;
/// use std::marker::PhantomData;
///
/// use mio_st::event::{Evented, EventedId};
/// use mio_st::poll::{Poll, PollOpt, Ready, Private};
/// use mio_st::registration::{Registration, Notifier};
///
/// /// Create a new channel.
/// fn new_channel<T>() -> (Sender<T>, Receiver<T>) {
///     // Create a new user space registration.
///     let (registration, notifier) = Registration::new();
///     (Sender {
///         notifier,
///         _phantom: PhantomData,
///     }, Receiver {
///         registration,
///         _phantom: PhantomData,
///     })
/// }
///
/// /// The receiving end of a channel.
/// pub struct Receiver<T> {
///     registration: Registration,
///     _phantom: PhantomData<T>,
/// }
///
/// impl<T> Receiver<T> {
///     /// Try to receiving a value from the channel, returning `None` if it is
///     /// empty.
///     fn try_receive(&mut self) -> Option<T> {
///         // Receive value etc.
/// #       unimplemented!();
///     }
/// }
///
/// // Deligate the Evented registration to the user space registration.
/// impl<T> Evented for Receiver<T> {
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
///         self.registration.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
///         self.registration.reregister(poll, id, interests, opt, p)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
///         self.registration.deregister(poll, p)
///     }
/// }
///
/// /// The sending end of a channel.
/// pub struct Sender<T> {
///     notifier: Notifier,
///     _phantom: PhantomData<T>,
/// }
///
/// impl<T> Sender<T> {
///     /// Send a new value across the channel.
///     fn send(&mut self, poll: &mut Poll, value: T) {
///         // Send value etc.
///
///         // Notify the receiving end of a new value.
///         self.notifier.notify(poll, Ready::READABLE);
/// #       unimplemented!();
///     }
/// }
/// ```
pub trait Evented {
    /// Register `self` with the given `Poll` instance.
    ///
    /// This function should not be called directly, use [`Poll.register`]
    /// instead.
    ///
    /// [`Poll.register`]: ../poll/struct.Poll.html#method.register
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()>;

    /// Reregister `self` with the given `Poll` instance.
    ///
    /// This function should not be called directly, use [`Poll.reregister`]
    /// instead.
    ///
    /// [`Poll.reregister`]: ../poll/struct.Poll.html#method.reregister
    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()>;

    /// Deregister `self` from the given `Poll` instance
    ///
    /// This function should not be called directly, use [`Poll.deregister`]
    /// instead.
    ///
    /// [`Poll.deregister`]: ../poll/struct.Poll.html#method.deregister
    fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()>;
}

/// An iterator over a collection of readiness events.
///
/// `Events` is passed as an argument to [`Poll.poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created at the same time as a [`Poll`] and
/// reused on each call to [`Poll.poll`].
///
/// See [`Poll`] for more documentation on polling.
///
/// [`Poll.poll`]: ../struct.Poll.html#method.poll
/// [`Poll`]: ../struct.Poll.html
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{EventedId, Events};
/// use mio_st::poll::{Poll, PollOpt, Ready};
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(128, 128);
///
/// // Register `Evented` handles with `poll` here.
///
/// // Run the event loop.
/// loop {
///     poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
///     for event in &mut events {
///         println!("event={:?}", event);
///     }
/// #   return Ok(());
/// }
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct Events {
    /// System events created by the system selector.
    sys_events: sys::Events,
    /// User space events created by `Poll` internally, this including timers.
    user_events: Vec<Event>,
    /// The position is used for both the system and user events. If `pos` >=
    /// `sys_events.len()`, then `pos - sys_events.len()` is used to index
    /// `user_events`.
    pos: usize,
}

impl Events {
    /// Create a new `Events` collection, this is split in capacity for system
    /// events (coming from epoll, kqueue etc.) and one coming from user space
    /// events (coming from user [`Registration`]s and deadlines).
    ///
    /// [`Registration`]: ../registration/struct.Registration.html
    pub fn with_capacity(system_capacity: usize, user_capacity: usize) -> Events {
        Events {
            sys_events: sys::Events::with_capacity(system_capacity),
            user_events: Vec::with_capacity(user_capacity),
            pos: 0,
        }
    }

    /// Returns the number of events in this iteration.
    pub fn len(&self) -> usize {
        self.sys_events.len() + self.user_events.len()
    }

    /// Whether or not this iteration is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reset the events to allow it to be filled again.
    pub(crate) fn reset(&mut self) {
        self.sys_events.clear();
        self.user_events.clear();
        self.pos = 0;
    }

    /// Get a mutable reference to the internal system events.
    pub(crate) fn system_events_mut(&mut self) -> &mut sys::Events {
        &mut self.sys_events
    }

    /// Add an user space event.
    pub(crate) fn push(&mut self, event: Event) {
        self.user_events.push(event);
    }

    /// Extend the user space events.
    pub(crate) fn extend_events(&mut self, events: &[Event]) {
        self.user_events.extend_from_slice(events);
    }

    fn user_pos(&self) -> usize {
        self.pos - self.sys_events.len()
    }
}

impl<'a> Iterator for &'a mut Events {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        // First try the system events, the user space events.
        let ret = match self.sys_events.get(self.pos) {
            Some(event) => Some(event),
            None => self.user_events.get(self.user_pos()).cloned(),
        };
        self.pos += 1;
        ret
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for &'a mut Events {
    fn len(&self) -> usize {
        // & &mut self -> & self.
        (&**self).len()
    }
}

/// An readiness event.
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
