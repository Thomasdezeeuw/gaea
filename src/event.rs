//! Readiness event types and utilities.

use std::io;

use sys;
use poll::{Poll, PollOpt, Ready, Token};

/// A value that may be registered with `Poll`.
///
/// Values that implement `Evented` can be registered with [`Poll`]. Users of
/// Mio should not use the `Evented` trait functions directly. Instead, the
/// equivalent functions on [`Poll`] should be used.
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
/// # Examples
///
/// Implementing `Evented` on a struct containing a system handle, such as a
/// [`TcpStream`].
///
/// ```
/// use std::io;
///
/// use mio::event::{Evented, EventedId};
/// use mio::net::TcpStream;
/// use mio::poll::{Poll, PollOpt, Ready};
///
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poll: &mut Poll, token: EventedId, interest: Ready, opts: PollOpt) -> io::Result<()> {
///         // Delegate the `register` call to `socket`
///         self.socket.register(poll, token, interest, opts)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, token: EventedId, interest: Ready, opts: PollOpt) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`
///         self.socket.reregister(poll, token, interest, opts)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
///         // Delegate the `deregister` call to `socket`
///         self.socket.deregister(poll)
///     }
/// }
/// ```
///
/// Implementing `Evented` using a user space handle, using [`Registration`] and
/// [`Notifier`],
///
/// ```
/// use std::io;
/// # use std::marker::PhantomData;
///
/// use mio::event::{Evented, EventedId};
/// use mio::poll::{Poll, PollOpt, Ready};
/// use mio::registration::{Registration, Notifier};
///
/// /// Create a new channel.
/// fn new_channel<T>() -> (Sender<T>, Receiver<T>) {
///     // Create a new user space registration.
///     let (registration, notifier) = Registration::new();
///     (Sender {
///         notifier,
/// #       _phantom: PhantomData,
///     }, Receiver {
///         registration,
/// #       _phantom: PhantomData,
///     })
/// }
///
/// /// The receiving end of a channel.
/// pub struct Receiver<T> {
///     registration: Registration,
///     # _phantom: PhantomData<T>,
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
///     fn register(&mut self, poll: &mut Poll, token: EventedId, interest: Ready, opts: PollOpt) -> io::Result<()> {
///         self.registration.register(poll, token, interest, opts)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, token: EventedId, interest: Ready, opts: PollOpt) -> io::Result<()> {
///         self.registration.reregister(poll, token, interest, opts)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
///         self.registration.deregister(poll)
///     }
/// }
///
/// /// The sending end of a channel.
/// pub struct Sender<T> {
///     notifier: Notifier,
/// #   _phantom: PhantomData<T>,
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
    /// [`Poll.register`]: ../struct.Poll.html#method.register
    fn register(&mut self, poll: &mut Poll, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()>;

    /// Reregister `self` with the given `Poll` instance.
    ///
    /// This function should not be called directly, use [`Poll.reregister`]
    /// instead.
    ///
    /// [`Poll.reregister`]: ../struct.Poll.html#method.reregister
    fn reregister(&mut self, poll: &mut Poll, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()>;

    /// Deregister `self` from the given `Poll` instance
    ///
    /// This function should not be called directly, use [`Poll.deregister`]
    /// instead.
    ///
    /// [`Poll.deregister`]: ../struct.Poll.html#method.deregister
    fn deregister(&mut self, poll: &mut Poll) -> io::Result<()>;
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
/// use mio::poll::{Poll, Token, Ready, PollOpt};
/// use mio::event::Events;
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(1024);
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
    inner: sys::Events,
    pos: usize,
}

impl Events {
    /// Create a new `Events` collection with the provided `capacity`.
    pub fn with_capacity(capacity: usize) -> Events {
        Events {
            inner: sys::Events::with_capacity(capacity),
            pos: 0,
        }
    }

    /// Reset the events to allow it to be filled again.
    pub(crate) fn reset(&mut self) {
        self.inner.clear();
        self.pos = 0;
    }

    /// Gain access to the internal `sys::Events`.
    pub(crate) fn inner_mut(&mut self) -> &mut sys::Events {
        &mut self.inner
    }

    /// Add an event.
    pub(crate) fn push(&mut self, event: Event) {
        self.inner.push_event(event);
    }

    /// Extend the events with the provided `extra` events.
    pub(crate) fn extend_events(&mut self, extra: &[Event]) {
        self.inner.extend_events(extra);
    }
}

impl<'a> Iterator for &'a mut Events {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        let ret = self.inner.get(self.pos);
        self.pos += 1;
        ret
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.inner.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for &'a mut Events {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// An readiness event.
///
/// `Event` is a [readiness state] paired with a [`Token`]. It is returned by
/// [`Poll.poll`].
///
/// For more documentation on polling and events, see [`Poll`].
///
/// # Examples
///
/// ```
/// use mio::{Ready, Token};
/// use mio::event::Event;
///
/// let event = Event::new(Token(0), Ready::READABLE | Ready::WRITABLE);
///
/// assert_eq!(event.readiness(), Ready::READABLE | Ready::WRITABLE);
/// assert_eq!(event.token(), Token(0));
/// ```
///
/// [readiness state]: ../struct.Ready.html
/// [`Token`]: ../struct.Token.html
/// [`Poll.poll`]: ../struct.Poll.html#method.poll
/// [`Poll`]: ../struct.Poll.html
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Event {
    token: Token,
    readiness: Ready,
}

impl Event {
    /// Creates a new `Event` containing `token` and `readiness`.
    pub fn new(token: Token, readiness: Ready) -> Event {
        Event { token, readiness }
    }

    /// Returns the event's readiness.
    pub fn readiness(&self) -> Ready {
        self.readiness
    }

    /// Returns the event's token.
    pub fn token(&self) -> Token {
        self.token
    }

    /// Gain access to kind of event.
    pub(crate) fn kind_mut(&mut self) -> &mut Ready {
        &mut self.readiness
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
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poll.register`]: struct.Poll.html#method.register
/// [`Poll.reregister`]: struct.Poll.html#method.reregister
/// [`Event`]: ../event/struct.Event.html
/// [`Poll`]: struct.Poll.html
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventedId(pub usize);

/// The only invalid token.
///
/// [`EventedId.is_valid`] can be used to determine if the token valid.
pub(crate) const INVALID_EVENTED_ID: EventedId = EventedId(::std::usize::MAX);

impl EventedId {
    /// Wether or not the `EventedId` is valid.
    pub fn is_valid(&self) -> bool {
        *self != INVALID_EVENTED_ID
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        if !self.is_valid() {
            Err(io::Error::new(io::ErrorKind::Other, "invalid evented id"))
        } else {
            Ok(())
        }
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
