//! Operating System backed readiness event queue.
//!
//! [`OsQueue`] provides an abstraction over platform specific Operating System
//! backed readiness event queues, such as kqueue or epoll.
//!
//! [`OsQueue`]: crate::os::OsQueue
//!
//! # Portability
//!
//! Using [`OsQueue`] provides a portable interface across supported platforms
//! as long as the caller takes the following into consideration:
//!
//! ### Draining readiness
//!
//! When using [edge-triggered] mode, once a readiness event is received, the
//! corresponding operation must be performed repeatedly until it returns
//! [`WouldBlock`]. Unless this is done, there is no guarantee that another
//! readiness event will be delivered, even if further data is received for the
//! [`Evented`] handle. See [`PollOption`] for more.
//!
//! [`WouldBlock`]: std::io::ErrorKind::WouldBlock
//! [edge-triggered]: crate::os::PollOption::Edge
//! [`Evented`]: crate::os::Evented
//! [`PollOption`]: crate::os::PollOption
//!
//! ### Spurious events
//!
//! The [`Source::poll`] implementation may return readiness events even if the
//! associated [`Evented`] handle is not actually ready. Given the same code,
//! this may happen more on some platforms than others. It is important to never
//! assume that, just because a readiness notification was received, that the
//! associated operation will as well.
//!
//! If operation fails with a [`WouldBlock`] error, then the caller should not
//! treat this as an error and wait until another readiness event is received.
//!
//! [`Source::poll`]: crate::event::Source::poll
//!
//! ### Registering handles
//!
//! Unless otherwise noted, it should be assumed that types implementing
//! [`Evented`] will never become ready unless they are registered with
//! `OsQueue`.
//!
//! For example:
//!
//! ```
//! # fn main() -> Result<(), Box<std::error::Error>> {
//! use std::thread;
//! use std::time::Duration;
//!
//! use mio_st::event;
//! use mio_st::net::TcpStream;
//! use mio_st::os::{OsQueue, PollOption};
//!
//! let address = "216.58.193.100:80".parse()?;
//! let mut stream = TcpStream::connect(address)?;
//!
//! // This actually does nothing towards connecting the TCP stream.
//! thread::sleep(Duration::from_secs(1));
//!
//! let mut os_queue = OsQueue::new()?;
//!
//! // The connect is not guaranteed to have started until it is registered at
//! // this point.
//! os_queue.register(&mut stream, event::Id(0), TcpStream::INTERESTS, PollOption::Edge)?;
//! #     Ok(())
//! # }
//! ```
//!
//! # Implementation notes
//!
//! `OsQueue` is backed by a readiness event queue provided by the operating
//! system. On all platforms a call to [`Source::poll`] is mostly just a direct
//! system call. The following system implementations back `OsQueue`:
//!
//! | OS      | Selector |
//! |---------|----------|
//! | FreeBSD | [kqueue](https://www.freebsd.org/cgi/man.cgi?query=kqueue) |
//! | Linux   | [epoll](http://man7.org/linux/man-pages/man7/epoll.7.html) |
//! | Mac OS  | [kqueue](https://developer.apple.com/legacy/library/documentation/Darwin/Reference/ManPages/man2/kqueue.2.html) |
//! | NetBSD  | [kqueue](http://netbsd.gw.com/cgi-bin/man-cgi?kqueue) |
//! | OpenBSD | [kqueue](https://man.openbsd.org/kqueue) |
//!
//! On all supported platforms socket operations are handled by using the system
//! queue. Platform specific extensions (e.g. [`EventedFd`]) allow accessing
//! other features provided by individual system selectors. For example Linux's
//! [`signalfd`] feature can be used by registering the file descriptor with
//! `OsQueue` via [`EventedFd`].
//!
//!
//! [`Eventedfd`]: crate::sys::unix::EventedFd
//! [`signalfd`]: http://man7.org/linux/man-pages/man2/signalfd.2.html

use std::io;
use std::time::Duration;

use log::trace;

use crate::event::{self, Events};
use crate::sys;

mod awakener;
mod evented;
mod interests;
mod option;

pub use self::awakener::Awakener;
pub use self::evented::Evented;
pub use self::interests::Interests;
pub use self::option::PollOption;

/// Readiness event queue backed by the OS.
///
/// This queue allows a program to monitor a large number of [`Evented`]
/// handles, waiting until one or more become "ready" for some class of
/// operations; e.g. [reading] or [writing]. An [`Evented`] type is considered
/// ready if it is possible to immediately perform a corresponding operation;
/// e.g. read or write.
///
/// To use this queue an [`Evented`] handle must first be registered using the
/// [`register`] method, supplying an associated id, readiness interests and
/// polling option. The [associated id] is used to associate a readiness event
/// with an `Evented` handle. The readiness [interests] defines which specific
/// operations on the handle to monitor for readiness. And the final argument,
/// [`PollOption`], defines how to deliver the readiness events, see
/// [`PollOption`] for more information.
///
/// See to [module documentation] for information.
///
/// [reading]: crate::event::Ready::READABLE
/// [writing]: crate::event::Ready::WRITABLE
/// [`register`]: OsQueue::register
/// [associated id]: event::Id
/// [interests]: Interests
/// [module documentation]: crate::os
#[derive(Debug)]
pub struct OsQueue {
    selector: sys::Selector,
}

impl OsQueue {
    /// Create a new OS backed readiness event queue.
    ///
    /// This function will make a syscall to the operating system to create the
    /// system selector. If this syscall fails it will return the error.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::os::OsQueue;
    /// use mio_st::poll;
    ///
    /// // Create a new OS backed readiness event queue.
    /// let mut os_queue = OsQueue::new()?;
    ///
    /// // Create an event container.
    /// let mut events = Vec::new();
    ///
    /// // Poll the queue for new readiness events.
    /// // But since no `Evented` handles have been registered we'll receive no
    /// // events.
    /// poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(500)))?;
    /// #     Ok(())
    /// # }
    /// ```
    pub fn new() -> io::Result<OsQueue> {
        sys::Selector::new().map(|selector| OsQueue { selector })
    }

    /// Register an [`Evented`] handle with the `OsQueue`.
    ///
    /// Once registered, the [`Evented`] handle will be monitored for readiness
    /// state changes. When it notices a state change, it will return a
    /// readiness event for the handle the next time the queue is [`polled`].
    ///
    /// [`polled`]: fn@crate::poll
    ///
    /// # Arguments
    ///
    /// `handle`: This is the handle that the `OsQueue` should monitor for
    /// readiness state changes.
    ///
    /// `id`: The caller picks a id to associate with the handle. When [`poll`]
    /// returns an [event] for the handle, this id is [included]. This allows
    /// the caller to map the event to its handle. The id associated with the
    /// `Evented` handle can be changed at any time by calling [`reregister`].
    ///
    /// `interests`: Specifies which operations `OsQueue` should monitor for
    /// readiness. `OsQueue` will only return readiness events for operations
    /// specified by this argument. If a socket is registered with [readable]
    /// interests and the socket becomes writable, no event will be returned
    /// from [`poll`]. The readiness interests for an `Evented` handle can be
    /// changed at any time by calling [`reregister`]. Most types that
    /// implemented [`Evented`] have a associated constant named `INTERESTS`
    /// which provide a sane interest for that type, e.g. [`TcpStream`
    /// interests] are readable and writable.
    ///
    /// `opt`: Specifies the registration option. Just like the interests and
    /// id, the option can be changed for an `Evented` handle at any time by
    /// calling [`reregister`].
    ///
    /// [`poll`]: fn@crate::poll
    /// [event]: crate::event::Event
    /// [included]: crate::event::Event::id
    /// [`reregister`]: OsQueue::reregister
    /// [readable]: Interests::READABLE
    /// [`TcpStream` interests]: crate::net::TcpStream::INTERESTS
    ///
    /// # Notes
    ///
    /// Unless otherwise specified, the caller should assume that once an
    /// `Evented` handle is registered with a `OsQueue` instance, it is bound to
    /// that `OsQueue` for the lifetime of the `Evented` handle. This remains
    /// true even if the `Evented` handle is [deregistered].
    ///
    /// [deregistered]: OsQueue::deregister
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use mio_st::net::TcpStream;
    /// use mio_st::os::{OsQueue, PollOption};
    /// use mio_st::{event, poll};
    ///
    /// // Create a new `OsQueue` as well a containers for the events.
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with queue.
    /// os_queue.register(&mut stream, event::Id(0), TcpStream::INTERESTS, PollOption::Edge)?;
    ///
    /// // Run the event loop.
    /// loop {
    ///     poll(&mut os_queue, &mut [], &mut events, None)?;
    ///
    ///     for event in events.drain(..) {
    ///         if event.id() == event::Id(0) {
    ///             // The TCP connection is (likely) ready for use.
    ///             # return Ok(());
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn register<E>(&mut self, handle: &mut E, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()>
        where E: Evented + ?Sized,
    {
        trace!("registering handle: id={}, interests={:?}, opt={:?}", id, interests, opt);
        handle.register(self, id, interests, opt)
    }

    /// Re-register an `Evented` handle with `OsQueue`.
    ///
    /// Re-registering an `Evented` handle allows changing the details of the
    /// registration. Specifically, it allows updating the associated `id`,
    /// `interests`, and `opt` specified in previous `register` and `reregister`
    /// calls.
    ///
    /// The `reregister` arguments **fully override** the previous values. In
    /// other words, if a socket is registered with [readable] interest and the
    /// call to `reregister` specifies only [writable], then read interest is no
    /// longer monitored for the handle.
    ///
    /// The `Evented` handle must have previously been registered with this
    /// `OsQueue` otherwise the call to `reregister` may return an error.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments.
    ///
    /// [readable]: Interests::READABLE
    /// [writable]: Interests::WRITABLE
    /// [`register`]: OsQueue::register
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use mio_st::{event, poll};
    /// use mio_st::net::TcpStream;
    /// use mio_st::os::{Interests, PollOption, OsQueue};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `OsQueue`, only with readable interest.
    /// os_queue.register(&mut stream, event::Id(0), Interests::READABLE, PollOption::Edge)?;
    ///
    /// // Reregister the connection specifying a different id and write interest
    /// // instead. `PollOption::Edge` must be specified even though that value
    /// // is not being changed.
    /// os_queue.reregister(&mut stream, event::Id(2), Interests::WRITABLE, PollOption::Edge)?;
    ///
    /// // Run the event loop.
    /// loop {
    ///     poll(&mut os_queue, &mut [], &mut events, None)?;
    ///
    ///     for event in events.drain(..) {
    ///         if event.id() == event::Id(2) {
    ///             // The TCP connection is (likely) ready for use.
    ///             # return Ok(());
    ///         } else if event.id() == event::Id(0) {
    ///             // We won't receive events with the old id anymore.
    ///             unreachable!();
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn reregister<E>(&mut self, handle: &mut E, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()>
        where E: Evented + ?Sized,
    {
        trace!("reregistering handle: id={}, interests={:?}, opt={:?}", id, interests, opt);
        handle.reregister(self, id, interests, opt)
    }

    /// Deregister an `Evented` handle from `OsQueue`.
    ///
    /// When an `Evented` handle is deregistered, the handle will no longer be
    /// monitored for readiness state changes. Unlike disabling handles with
    /// [`oneshot`], deregistering clears up any internal resources needed to
    /// track the handle.
    ///
    /// A handle can be registered again using [`register`] after it has been
    /// deregistered; however, it must be passed back to the **same** `OsQueue`.
    ///
    /// # Notes
    ///
    /// Calling [`reregister`] after `deregister` may be work on some platforms
    /// but not all. To properly re-register a handle after deregistering use
    /// `register`, this works on all platforms.
    ///
    /// [`oneshot`]: PollOption::Oneshot
    /// [`register`]: OsQueue::register
    /// [`reregister`]: OsQueue::reregister
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::{event, poll};
    /// use mio_st::net::TcpStream;
    /// use mio_st::os::{OsQueue, PollOption};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `OsQueue`.
    /// os_queue.register(&mut stream, event::Id(0), TcpStream::INTERESTS, PollOption::Edge)?;
    ///
    /// // Do stuff with the connection etc.
    ///
    /// // Deregister it so the resources can be cleaned up.
    /// os_queue.deregister(&mut stream)?;
    ///
    /// // Set a timeout because we shouldn't receive any events anymore.
    /// poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(100)))?;
    /// assert!(events.is_empty());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn deregister<E>(&mut self, handle: &mut E) -> io::Result<()>
        where E: Evented + ?Sized,
    {
        trace!("deregistering handle");
        handle.deregister(self)
    }

    /// Get access to the system selector. Used by platform specific code, e.g.
    /// `EventedFd`.
    pub(crate) fn selector(&self) -> &sys::Selector {
        &self.selector
    }
}

impl<Evts> event::Source<Evts> for OsQueue
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        // Can't tell if an event is available.
        None
    }

    fn poll(&mut self, events: &mut Evts) -> io::Result<()> {
        use crate::event::BlockingSource;
        self.blocking_poll(events, Some(Duration::from_millis(0)))
    }
}

impl<Evts> event::BlockingSource<Evts> for OsQueue
    where Evts: Events,
{
    fn blocking_poll(&mut self, events: &mut Evts, timeout: Option<Duration>) -> io::Result<()> {
        trace!("polling OS selector: timeout={:?}", timeout);
        self.selector.select(events, timeout)
    }
}
