//! Readiness event types and utilities.

use std::{cmp, io, ptr};

use arrayvec::ArrayVec;

use poll::{Poll, PollCalled, PollOpt, Ready};

mod id;

pub use self::id::EventedId;

pub(crate) use self::id::INVALID_EVENTED_ID;

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
/// use mio_st::poll::{Poll, PollOpt, Ready, PollCalled};
///
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()> {
///         // Delegate the `register` call to `socket`
///         self.socket.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`
///         self.socket.reregister(poll, id, interests, opt, p)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll, p: PollCalled) -> io::Result<()> {
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
/// use mio_st::poll::{Poll, PollOpt, Ready, PollCalled};
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
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()> {
///         self.registration.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()> {
///         self.registration.reregister(poll, id, interests, opt, p)
///     }
///
///     fn deregister(&mut self, poll: &mut Poll, p: PollCalled) -> io::Result<()> {
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
///     fn send(&mut self, value: T) {
///         // Send value etc.
///
///         // Notify the receiving end of a new value.
///         self.notifier.notify(Ready::READABLE);
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
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()>;

    /// Reregister `self` with the given `Poll` instance.
    ///
    /// This function should not be called directly, use [`Poll.reregister`]
    /// instead.
    ///
    /// [`Poll.reregister`]: ../poll/struct.Poll.html#method.reregister
    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: PollCalled) -> io::Result<()>;

    /// Deregister `self` from the given `Poll` instance
    ///
    /// This function should not be called directly, use [`Poll.deregister`]
    /// instead.
    ///
    /// [`Poll.deregister`]: ../poll/struct.Poll.html#method.deregister
    fn deregister(&mut self, poll: &mut Poll, p: PollCalled) -> io::Result<()>;
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
/// let mut events = Events::new();
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
    /// Stack allocted events.
    events: ArrayVec<[Event; 512]>,
    /// Position of the iterator.
    pos: usize,
}

impl Events {
    /// Create a new `Events` collection.
    ///
    /// # Notes
    ///
    /// Internally there is *currently* a maximum capacity of 512 events. At
    /// most 256 events will be used for system events.
    pub fn new() -> Events {
        Events { events: ArrayVec::new(), pos: 0 }
    }

    /// Returns the number of events in this iteration.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether or not this iteration is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Reset the events to allow it to be filled again.
    pub(crate) fn reset(&mut self) {
        // This is safe because `Event` doesn't implement `Drop`.
        unsafe { self.events.set_len(0); }
        self.pos = 0;
    }

    /// Returns the capacity.
    pub(crate) fn capacity(&self) -> usize {
        self.events.capacity()
    }

    /// Add an user space event.
    pub(crate) fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Extend the events, returns the number of events added.
    pub(crate) fn extend_events(&mut self, events: &[Event]) -> usize {
        let count = cmp::min(self.capacity_left(), events.len());
        if count == 0 {
            return 0;
        }

        let len = self.len();
        unsafe {
            let dst = self.events.as_mut_ptr().offset(len as isize);
            ptr::copy_nonoverlapping(events.as_ptr(), dst, count);
            self.events.set_len(len + count);
        }

        count
    }

    /// Returns the leftover capacity.
    pub(crate) fn capacity_left(&self) -> usize {
        self.events.capacity() - self.events.len()
    }
}

impl Default for Events {
    fn default() -> Events {
        Events::new()
    }
}

impl<'a> Iterator for &'a mut Events {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        let ret = self.events.get(self.pos).cloned();
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
