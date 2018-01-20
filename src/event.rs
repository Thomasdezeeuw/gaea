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
/// There are three types of `Evented` values.
///
/// * **System** handles, which are backed by sockets or other system handles.
/// These `Evented` handles will be monitored by the system selector. In this
/// case, an implementation of `Evented` delegates to a lower level handle, e.g.
/// unix's [`EventedFd`].
///
/// * **User** handles, which are driven entirely in user space using
/// [`Registration`] and [`SetReadiness`]. In this case, the implementer takes
/// responsibility for driving the readiness state changes.
///
/// * **Deadline** handles, these are internal handles and can only be used
/// using a [`Timer`].
///
/// [`Poll`]: ../poll/struct.Poll.html
/// [`Registration`]: ../registration/struct.Registration.html
/// [`Notifier`]: ../registration/struct.Notifier.html
/// [`EventedFd`]: ../unix/struct.EventedFd.html
/// [`Timer`]: ../timer/struct.Timer.html
///
/// # Examples
///
/// Implementing `Evented` on a struct containing a socket (a system handle).
///
/// ```
/// use mio::{Ready, Poll, PollOpt, Token};
/// use mio::event::Evented;
/// use mio::net::TcpStream;
///
/// use std::io;
///
/// pub struct MyEvented {
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
///         // Delegate the `register` call to `socket`
///         self.socket.register(poll, token, interest, opts)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
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
/// Implement `Evented` using [`Registration`] and [`Notifier`][] (user space
/// handle).
///
/// ```ignore
/// // TODO: add example
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
/// let event = Event::new(Ready::READABLE | Ready::WRITABLE, Token(0));
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
    kind: Ready,
    token: Token,
}

impl Event {
    /// Creates a new `Event` containing `readiness` and `token`.
    pub fn new(readiness: Ready, token: Token) -> Event {
        Event {
            kind: readiness,
            token: token,
        }
    }

    /// Returns the event's readiness.
    pub fn readiness(&self) -> Ready {
        self.kind
    }

    /// Returns the event's token.
    pub fn token(&self) -> Token {
        self.token
    }

    /// Gain access to kind of event.
    pub(crate) fn kind_mut(&mut self) -> &mut Ready {
        &mut self.kind
    }
}
