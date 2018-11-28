//! Types related to polling.
//!
//! The main type is [`Poller`], see it as well as the [root of the crate] for
//! examples.
//!
//! [`Poller`]: struct.Poller.html
//! [root of the crate]: ../index.html

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};
use std::{io, mem};

use log::{trace, debug};

use crate::event::{Event, Evented, EventedId, Events, Ready};
use crate::sys;

mod interests;
mod option;

pub use self::interests::Interests;
pub use self::option::PollOption;

// Poller uses three subsystems to bring a complete event system to the user.
//
// 1. Operating System specific event queue. This is currently kqueue or epoll.
//    All the relevant code is in the `sys` module. This mainly deals with file
//    descriptors, e.g. for sockets.
//
// 2. User space events. This is simply a vector in the `Poller` instance,
//    adding an new event is a simple push onto it. `Events` hold both the
//    system events and user space events. Each call to `Poller.poll` simply
//    flushes all user space events to the provided `Events`.
//
// 3. Deadline system. The third subsystem is used for deadlines and timeouts.
//    Each deadline is a pair of `Instant` and `EventedId` in a binary heap.
//    Each call to `Poller.poll` will get the first deadline, if any, and use it
//    as a timeout to the system selector. Then after the system selector
//    returns exceeded deadlines are popped and converted into `Event`s and
//    added to the provided `Events`.

/// Polls for readiness events on all registered handles.
///
/// `Poller` allows a program to monitor a large number of [`Evented`] handles,
/// waiting until one or more become "ready" for some class of operations; e.g.
/// [reading] or [writing]. An `Evented` type is considered ready if it is
/// possible to immediately perform a corresponding operation; e.g. read or
/// write.
///
/// To use `Poller` an `Evented` handle must first be registered with the
/// `Poller` instance using the [`register`] method, supplying an associated id,
/// readiness interests and polling option. The associated id, or [`EventedId`],
/// is used to associate an readiness event with an `Evented` handle. The
/// readiness interests, or [`Ready`], tells `Poller` which specific operations
/// on the handle to monitor for readiness. And the final argument,
/// [`PollOption`], tells `Poller` how to deliver the readiness events, see
/// [`PollOption`] for more information.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [reading]: ../event/struct.Ready.html#associatedconstant.READABLE
/// [writing]: ../event/struct.Ready.html#associatedconstant.WRITABLE
/// [`register`]: #method.register
/// [`EventedId`]: ../event/struct.EventedId.html
/// [`Ready`]: ../event/struct.Ready.html
/// [`PollOption`]: enum.PollOption.html
///
/// # Portability
///
/// Using `Poller` provides a portable interface across supported platforms as
/// long as the caller takes the following into consideration:
///
/// ### Spurious events
///
/// [`Poller.poll`] may return readiness events even if the associated
/// [`Evented`] handle is not actually ready. Given the same code, this may
/// happen more on some platforms than others. It is important to never assume
/// that, just because a readiness notification was received, that the
/// associated operation will as well.
///
/// If operation fails with a [`WouldBlock`] error, then the caller should not
/// treat this as an error and wait until another readiness event is received.
///
/// ### Draining readiness
///
/// When using edge-triggered mode, once a readiness event is received, the
/// corresponding operation must be performed repeatedly until it returns
/// [`WouldBlock`]. Unless this is done, there is no guarantee that another
/// readiness event will be delivered, even if further data is received for the
/// [`Evented`] handle. See [`PollOption`] for more.
///
/// ### Readiness operations
///
/// The only readiness operations that are guaranteed to be present on all
/// supported platforms are [readable], [writable] and [timer]. All other
/// readiness operations may have false negatives and as such should be
/// considered **hints**. This means that if a socket is registered with
/// [readable], [error], and [hup] interest, and either an error or hup is
/// received, a readiness event will be generated for the socket, but it **may**
/// only include `readable` readiness. Also note that, given the potential for
/// spurious events, receiving a readiness event with `hup` or `error` doesn't
/// actually mean that a `read` on the socket will return a result matching the
/// readiness event.
///
/// In other words, portable programs that explicitly check for [hup] or [error]
/// readiness should be doing so as an **optimization** and always be able to
/// handle an error or HUP situation when performing the actual read operation.
///
/// ### Registering handles
///
/// Unless otherwise noted, it should be assumed that types implementing
/// [`Evented`] will never become ready unless they are registered with `Poller`.
///
/// For example:
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::thread;
/// use std::time::Duration;
///
/// use mio_st::event::{EventedId, Ready};
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poller, PollOption};
///
/// let address = "216.58.193.100:80".parse()?;
/// let mut stream = TcpStream::connect(address)?;
///
/// // This actually does nothing.
/// thread::sleep(Duration::from_secs(1));
///
/// let mut poll = Poller::new()?;
///
/// // The connect is not guaranteed to have started until it is registered at
/// // this point.
/// poll.register(&mut stream, EventedId(0), TcpStream::INTERESTS, PollOption::Edge)?;
/// #     Ok(())
/// # }
/// ```
///
/// [`Poller.poll`]: struct.Poller.html#method.poll
/// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
/// [readable]: ../event/struct.Ready.html#associatedconstant.READABLE
/// [writable]: ../event/struct.Ready.html#associatedconstant.WRITABLE
/// [error]: ../event/struct.Ready.html#associatedconstant.ERROR
/// [timer]: ../event/struct.Ready.html#associatedconstant.TIMER
/// [hup]: ../event/struct.Ready.html#associatedconstant.HUP
///
/// # Implementation notes
///
/// `Poller` is backed by the selector provided by the operating system.
///
/// | OS      | Selector |
/// |---------|----------|
/// | FreeBSD | [kqueue](https://www.freebsd.org/cgi/man.cgi?query=kqueue) |
/// | Linux   | [epoll](http://man7.org/linux/man-pages/man7/epoll.7.html) |
/// | Mac OS  | [kqueue](https://developer.apple.com/legacy/library/documentation/Darwin/Reference/ManPages/man2/kqueue.2.html) |
/// | NetBSD  | [kqueue](http://netbsd.gw.com/cgi-bin/man-cgi?kqueue) |
/// | OpenBSD | [kqueue](https://man.openbsd.org/kqueue) |
///
/// On all supported platforms socket operations are handled by using the system
/// selector. Platform specific extensions (e.g. [`EventedFd`]) allow accessing
/// other features provided by individual system selectors. For example Linux's
/// [`signalfd`] feature can be used by registering the file descriptor with
/// `Poller` via [`EventedFd`].
///
/// On all platforms a call to [`Poller.poll`] is mostly just a direct call to the
/// system selector, but it also adds user space and timer events. Notifications
/// generated by user space registration ([`Registration`] and [`Notifier`]) are
/// handled by an internal readiness queue. Deadlines and timers use the same
/// queue. A single call to [`Poller.poll`] will collect events from both from the
/// system selector and the internal readiness queue.
///
/// `Events` itself is split among system events and user space events,
/// including timers.
///
/// [`EventedFd`]: ../unix/struct.EventedFd.html
/// [`signalfd`]: http://man7.org/linux/man-pages/man2/signalfd.2.html
/// [`Registration`]: ../registration/struct.Registration.html
/// [`Notifier`]: ../registration/struct.Notifier.html
#[derive(Debug)]
pub struct Poller {
    selector: sys::Selector,
    userspace_events: Vec<Event>,
    deadlines: BinaryHeap<Reverse<Deadline>>,
}

impl Poller {
    /// Return a new `Poller` handle.
    ///
    /// This function will make a syscall to the operating system to create the
    /// system selector. If this syscall fails, `Poller::new` will return with
    /// the error.
    ///
    /// See [struct] level docs for more details.
    ///
    /// [struct]: struct.Poller.html
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::event::Events;
    /// use mio_st::poll::Poller;
    ///
    /// let mut poller = Poller::new()?;
    ///
    /// // Create a structure to receive polled events.
    /// let mut events = Events::new();
    ///
    /// // Wait for events, but none will be received because no `Evented`
    /// // handles have been registered with this `Poller` instance.
    /// poller.poll(&mut events, Some(Duration::from_millis(500)))?;
    /// #     Ok(())
    /// # }
    /// ```
    pub fn new() -> io::Result<Poller> {
        Ok(Poller {
            selector: sys::Selector::new()?,
            userspace_events: Vec::new(),
            deadlines: BinaryHeap::new(),
        })
    }

    /// Register an `Evented` handle with the `Poller` instance.
    ///
    /// Once registered, the `Poller` instance will monitor the [`Evented`]
    /// handle for readiness state changes. When it notices a state change, it
    /// will return a readiness event for the handle the next time [`poll`] is
    /// called.
    ///
    /// See the [`struct`] docs for a high level overview.
    ///
    /// # Arguments
    ///
    /// `handle`: This is the handle that the `Poller` instance should monitor
    /// for readiness state changes.
    ///
    /// `id`: The caller picks a id to associate with the handle. When [`poll`]
    /// returns an [event] for the handle, this id is [included]. This allows
    /// the caller to map the event to its handle. The id associated with the
    /// `Evented` handle can be changed at any time by calling [`reregister`].
    ///
    /// `interests`: Specifies which operations `Poller` should monitor for
    /// readiness. `Poller` will only return readiness events for operations
    /// specified by this argument. If a socket is registered with [readable]
    /// interests and the socket becomes writable, no event will be returned
    /// from [`poll`]. The readiness interests for an `Evented` handle can be
    /// changed at any time by calling [`reregister`]. Most types have a
    /// associated constant named `INTERESTS` which provide a good default
    /// value.
    ///
    /// `opt`: Specifies the registration option. Just like the interests, the
    /// option can be changed for an `Evented` handle at any time by calling
    /// [`reregister`].
    ///
    /// # Notes
    ///
    /// Unless otherwise specified, the caller should assume that once an
    /// `Evented` handle is registered with a `Poller` instance, it is bound to
    /// that `Poller` instance for the lifetime of the `Evented` handle. This
    /// remains true even if the `Evented` handle is deregistered from the poll
    /// instance using [`deregister`].
    ///
    /// [`Evented`]: ../event/trait.Evented.html
    /// [`poll`]: #method.poll
    /// [`struct`]: #
    /// [`reregister`]: #method.reregister
    /// [event]: ../event/struct.Event.html
    /// [included]: ../event/struct.Event.html#method.id
    /// [readable]: ../event/struct.Ready.html#associatedconstant.READABLE
    /// [`deregister`]: #method.deregister
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::{Duration, Instant};
    ///
    /// use mio_st::event::{Events, EventedId, Ready};
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Poller, PollOption};
    ///
    /// // Create a new `Poller` instance as well a containers for the vents.
    /// let mut poller = Poller::new()?;
    /// let mut events = Events::new();
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `poller`.
    /// poller.register(&mut stream, EventedId(0), TcpStream::INTERESTS, PollOption::Edge)?;
    ///
    /// // Start the event loop.
    /// loop {
    ///     poller.poll(&mut events, None)?;
    ///
    ///     for event in &mut events {
    ///         if event.id() == EventedId(0) {
    ///             // Connection is (likely) ready for use.
    ///             # return Ok(());
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn register<E>(&mut self, handle: &mut E, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>
        where E: Evented + ?Sized,
    {
        trace!("registering handle: id={}, interests={:?}, opt={:?}", id, interests, opt);
        handle.register(self, id, interests, opt)
    }

    /// Re-register an `Evented` handle with the `Poller` instance.
    ///
    /// Re-registering an `Evented` handle allows changing the details of the
    /// registration. Specifically, it allows updating the associated `id`,
    /// `interests`, and `opt` specified in previous `register` and `reregister`
    /// calls.
    ///
    /// The `reregister` arguments fully override the previous values. In other
    /// words, if a socket is registered with [readable] interest and the call
    /// to `reregister` specifies only [writable], then read interest is no
    /// longer requested for the handle.
    ///
    /// The `Evented` handle must have previously been registered with this
    /// instance of `Poller` otherwise the call to `reregister` may return an
    /// error.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments and see the [`struct`] docs for a high level overview of
    /// polling.
    ///
    /// [readable]: ../event/struct.Ready.html#associatedconstant.READABLE
    /// [writable]: ../event/struct.Ready.html#associatedconstant.WRITABLE
    /// [`register`]: #method.register
    /// [`struct`]: #
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use mio_st::event::EventedId;
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Interests, PollOption, Poller};
    ///
    /// let mut poller = Poller::new()?;
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `Poller`, only with readable interest.
    /// poller.register(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)?;
    ///
    /// // Reregister the connection specifying a different id and write interest
    /// // instead. `PollOption::Edge` must be specified even though that value
    /// // is not being changed.
    /// poller.reregister(&mut stream, EventedId(2), Interests::WRITABLE, PollOption::Edge)?;
    /// #     Ok(())
    /// # }
    /// ```
    pub fn reregister<E>(&mut self, handle: &mut E, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>
        where E: Evented + ?Sized,
    {
        trace!("reregistering handle: id={}, interests={:?}, opt={:?}", id, interests, opt);
        handle.reregister(self, id, interests, opt)
    }

    /// Deregister an `Evented` handle with the `Poller` instance.
    ///
    /// When an `Evented` handle is deregistered, the `Poller` instance will no
    /// longer monitor it for readiness state changes. Unlike disabling handles
    /// with [`oneshot`], deregistering clears up any internal resources needed
    /// to track the handle.
    ///
    /// A handle can be registered again using [`register`] after it has been
    /// deregistered; however, it must be passed back to the **same** `Poller`
    /// instance.
    ///
    /// # Notes
    ///
    /// Calling [`reregister`] after `deregister` may be work on some platforms
    /// but not all. To properly re-register a handle after deregistering use
    /// `register`, this works on all platforms.
    ///
    /// [`oneshot`]: enum.PollOption.html#variant.Oneshot
    /// [`register`]: #method.register
    /// [`reregister`]: #method.reregister
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::event::{Events, EventedId, Ready};
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Poller, PollOption};
    ///
    /// let mut poller = Poller::new()?;
    /// let mut events = Events::new();
    ///
    /// // Create a TCP connection. `TcpStream` implements the `Evented` trait.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `Poller`.
    /// poller.register(&mut stream, EventedId(0), TcpStream::INTERESTS, PollOption::Edge)?;
    ///
    /// // Do stuff with the connection etc.
    ///
    /// // Deregister it so the resources can be cleaned up.
    /// poller.deregister(&mut stream)?;
    ///
    /// // Set a timeout because this poller shouldn't receive any events anymore.
    /// poller.poll(&mut events, Some(Duration::from_millis(200)))?;
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

    /// Notify an evented handle of an user space event.
    ///
    /// This uses the user space event system.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::event::{Event, Events, EventedId, Ready};
    /// use mio_st::poll::Poller;
    ///
    /// let mut poller = Poller::new()?;
    /// let mut events = Events::new();
    ///
    /// // Add a custom user space notification.
    /// poller.notify(EventedId(0), Ready::READABLE);
    ///
    /// // Set a timeout because this poll should never receive any events.
    /// poller.poll(&mut events, None)?;
    /// assert_eq!((&mut events).next().unwrap(), Event::new(EventedId(0), Ready::READABLE));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn notify(&mut self, id: EventedId, ready: Ready) {
        trace!("adding user space event: id={}, ready={:?}", id, ready);
        self.userspace_events.push(Event::new(id, ready));
    }

    /// Add a new deadline to Poller.
    ///
    /// This will cause an event to trigger after the `deadline` has passed with
    /// the [`Ready::TIMER`] readiness and provided `id`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<std::error::Error>> {
    /// use std::time::{Duration, Instant};
    ///
    /// use mio_st::event::{Event, Events, EventedId, Ready};
    /// use mio_st::poll::Poller;
    ///
    /// // Our `Poller` instance and events.
    /// let mut poller = Poller::new()?;
    /// let mut events = Events::new();
    ///
    /// // Add our deadline, to trigger an event 10 milliseconds from now.
    /// let deadline = Instant::now() + Duration::from_millis(10);
    /// let id = EventedId(0);
    /// poller.add_deadline(id, deadline);
    ///
    /// // Even though we don't provide a timeout to poll this will return in
    /// // roughly 10 milliseconds and return an event with our deadline.
    /// poller.poll(&mut events, None)?;
    ///
    /// assert_eq!((&mut events).next(), Some(Event::new(id, Ready::TIMER)));
    /// #     Ok(())
    /// # }
    pub fn add_deadline(&mut self, id: EventedId, deadline: Instant) {
        trace!("adding deadline: id={}, deadline={:?}", id, deadline);
        self.deadlines.push(Reverse(Deadline { id, deadline }));
    }

    /// Remove a previously added deadline.
    ///
    /// # Notes
    ///
    /// Removing a deadline is a costly operation. For better performance it is
    /// advised to not bother with removing and instead ignore the event when it
    /// comes up.
    pub fn remove_deadline(&mut self, id: EventedId) {
        trace!("removing deadline: id={}", id);

        // TODO: optimize this.
        let index = self.deadlines.iter()
            .position(|deadline| deadline.0.id == id);

        if let Some(index) = index {
            let deadlines = mem::replace(&mut self.deadlines, BinaryHeap::new());
            let mut deadlines_vec = deadlines.into_vec();
            let removed_deadline = deadlines_vec.swap_remove(index);
            debug_assert_eq!(removed_deadline.0.id, id, "remove_deadline: removed incorrect deadline");
            drop(mem::replace(&mut self.deadlines, BinaryHeap::from(deadlines_vec)));
        }
    }

    /// Poll for readiness events.
    ///
    /// Blocks the current thread and waits for readiness events for any of the
    /// `Evented` handles that have been registered with this `Poller` instance
    /// previously.
    ///
    /// The function will block until either;
    ///
    /// - at least one readiness event has been received from,
    /// - a deadline is elapsed, or
    /// - the provided `timeout` has elapsed.
    ///
    /// Providing a `timeout` of `None` means that `poll` will block until one
    /// of the other two conditions are true. Note that the `timeout` will be
    /// rounded up to the system clock granularity (usually 1ms), and kernel
    /// scheduling delays mean that the blocking interval may be overrun by a
    /// small amount.
    ///
    /// The supplied `events` will be cleared and newly received readiness
    /// events will be stored in it. If not all events fit into the `events`,
    /// they will be returned on the next call to `poll`.
    ///
    /// A single call to `poll` may result in multiple readiness events being
    /// returned for a single `Evented` handle. For example, if a TCP socket
    /// becomes both readable and writable, it may be possible for a single
    /// readiness event to be returned with both [readable] and [writable]
    /// readiness **OR** two separate events may be returned, one with
    /// readable set and one with writable set.
    ///
    /// See the [struct] level documentation for a higher level discussion of
    /// polling.
    ///
    /// [readable]: ../event/struct.Ready.html#associatedconstant.READABLE
    /// [writable]: ../event/struct.Ready.html#associatedconstant.WRITABLE
    /// [struct]: #
    pub fn poll(&mut self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let mut timeout = self.determine_timeout(timeout);
        trace!("polling: timeout={:?}", timeout);

        events.clear();
        loop {
            let start = Instant::now();
            // Get the selector events.
            match self.selector.select(events, timeout) {
                Ok(()) => break,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                    debug!("polling interrupted, trying again");
                    // Interrupted by a signal; update timeout if necessary and
                    // retry.
                    if let Some(to) = timeout {
                        let elapsed = start.elapsed();
                        if elapsed >= to {
                            // Timeout elapsed so we need to return.
                            break;
                        } else {
                            timeout = Some(to - elapsed);
                        }
                    }
                },
                Err(e) => return Err(e),
            }
        }

        self.poll_userspace_internal(events);
        self.poll_deadlines(events);
        Ok(())
    }

    /// Poll for user space readiness events.
    ///
    /// The regular call to [`poll`] uses a system call to read readiness events
    /// for system resources such as sockets. This method **does not** do that,
    /// it will only read user space readiness events, including deadlines.
    ///
    /// Because no system call is used this method is faster then calling
    /// `poll`, even with a 0 ms timeout, and never blocks.
    ///
    /// [`poll`]: #method.poll
    pub fn poll_userspace(&mut self, events: &mut Events) {
        trace!("polling user space");
        events.clear();

        self.poll_userspace_internal(events);
        self.poll_deadlines(events);
    }

    /// Compute the timeout value to be passed to the system selector. If the
    /// user space queue has pending events, we still want to poll the system
    /// selector for new events, but we don't want to block the thread to wait
    /// for new events.
    ///
    /// If we have any deadlines the first one will also cap the timeout.
    fn determine_timeout(&mut self, timeout: Option<Duration>) -> Option<Duration> {
        if !self.userspace_events.is_empty() {
            // User space queue has events, so no blocking.
            return Some(Duration::from_millis(0));
        } else if let Some(deadline) = self.deadlines.peek() {
            let now = Instant::now();
            if deadline.0.deadline <= now {
                // Deadline has already expired, so no blocking.
                return Some(Duration::from_millis(0));
            }

            // Determine the timeout for the next deadline.
            let deadline_timeout = deadline.0.deadline.duration_since(now);
            match timeout {
                // The provided timeout is smaller then the deadline timeout, so
                // we'll keep the original timeout.
                Some(timeout) if timeout < deadline_timeout => {},
                // Deadline timeout is sooner, use that.
                _ => return Some(deadline_timeout),
            }
        }

        timeout
    }

    /// Poll user space events.
    fn poll_userspace_internal(&mut self, events: &mut Events) {
        trace!("polling user space events");
        let n = events.extend_events(&self.userspace_events);
        if self.userspace_events.len() - n == 0 {
            self.userspace_events.clear();
        } else {
            drop(self.userspace_events.drain(..n));
        }
    }

    /// Add expired deadlines to the provided `events`.
    fn poll_deadlines(&mut self, events: &mut Events) {
        trace!("polling deadlines");
        let now = Instant::now();

        for _ in 0..events.capacity_left() {
            match self.deadlines.peek() {
                Some(deadline) if deadline.0.deadline <= now => {
                    let deadline = self.deadlines.pop().unwrap().0;
                    events.push(Event::new(deadline.id, Ready::TIMER));
                },
                _ => return,
            }
        }
    }

    /// Get access to the system selector. Used by platform specific code, e.g.
    /// `EventedFd`.
    pub(crate) fn selector(&self) -> &sys::Selector {
        &self.selector
    }
}

/// A deadline in `Poller`.
///
/// This must be ordered by `deadline`, then `id`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Deadline {
    deadline: Instant,
    id: EventedId,
}
