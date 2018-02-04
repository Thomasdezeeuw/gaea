//! Types related to polling.
//!
//! The main type is [`Poll`], see it as well as the [root of the crate] for
//! examples.
//!
//! [`Poll`]: struct.Poll.html
//! [root of the crate]: ../index.html

use std::{io, mem, ops, ptr};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use std::collections::BinaryHeap;

use sys;
use event::{Event, EventedId, Events, Evented};

mod opt;
mod ready;

pub use self::opt::PollOpt;
pub use self::ready::Ready;

// Poll uses three subsystems to bring a complete event system to the user.
//
// 1. Operating System specific event queue. This is currently kqueue or epoll.
//    All the relavent code is in the `sys` module. This mainly deals with file
//    descriptor, e.g. for sockets.
//
// 2. User space events. This is simply a vector in the `Poll` instance. Adding
//    an new events is a simple a push it onto the vector. `events::Events` hold
//    both the system events and user space events. Each call to `Poll.poll`
//    simply flushes all user space events to the provided `events::Events`.
//
// 3. Deadline system. The third system is used for deadlines and timeout. Each
//    deadline is a pair of `Instant` and `EventedId` in a binary heap. Each
//    call to `Poll.poll` will get the first deadline, if any, and use it as a
//    timeout to the system selector. Then after the system selector returns
//    exceeded deadlines are popped and converted into Events and added to the
//    user space events.

/// Polls for readiness events on all registered handles.
///
/// `Poll` allows a program to monitor a large number of [`Evented`] handles,
/// waiting until one or more become "ready" for some class of operations; e.g.
/// [reading] or [writing]. An `Evented` type is considered ready if it is
/// possible to immediately perform a corresponding operation; e.g. read or
/// write.
///
/// To use `Poll` an `Evented` handle must first be registered with the `Poll`
/// instance using the [`register`] method, supplying an associated id,
/// readiness interests and polling option. The associated id, or [`EventedId`],
/// is used to associate an readiness event with an `Evented` handle. The
/// readiness interests, or [`Ready`], tells `Poll` which specific operations on
/// the handle to monitor for readiness. And the final argument, [`PollOpt`],
/// tells `Poll` how to deliver the readiness events, see [`PollOpt`] for more
/// information.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [reading]: struct.Ready.html#associatedconstant.READABLE
/// [writing]: struct.Ready.html#associatedconstant.WRITABLE
/// [`register`]: #method.register
/// [`EventedId`]: ../event/struct.EventedId.html
/// [`Ready`]: struct.Ready.html
/// [`PollOpt`]: enum.PollOpt.html
///
/// # Portability
///
/// Using `Poll` provides a portable interface across supported platforms as
/// long as the caller takes the following into consideration:
///
/// ### Spurious events
///
/// [`Poll.poll`] may return readiness events even if the associated
/// [`Evented`] handle is not actually ready. Given the same code, this may
/// happen more on some platforms than others. It is important to never assume
/// that, just because a readiness notification was received, that the
/// associated operation will as well.
///
/// If operation fails with a [`WouldBlock`] error, then the caller should not treat
/// this as an error and wait until another readiness event is received.
///
/// ### Draining readiness
///
/// When using edge-triggered mode, once a readiness event is received, the
/// corresponding operation must be performed repeatedly until it returns
/// [`WouldBlock`]. Unless this is done, there is no guarantee that another
/// readiness event will be delivered, even if further data is received for the
/// [`Evented`] handle. See [`PollOpt`] for more.
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
/// [`Evented`] will never become ready unless they are registered with `Poll`.
///
/// For example:
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::thread;
/// use std::time::Duration;
///
/// use mio_st::event::EventedId;
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poll, Ready, PollOpt};
///
/// let address = "216.58.193.100:80".parse()?;
/// let mut stream = TcpStream::connect(address)?;
///
/// // This actually does nothing.
/// thread::sleep(Duration::from_secs(1));
///
/// let mut poll = Poll::new()?;
///
/// // The connect is not guaranteed to have started until it is registered at
/// // this point.
/// poll.register(&mut stream, EventedId(0), Ready::READABLE | Ready::WRITABLE, PollOpt::Edge)?;
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`Poll.poll`]: struct.Poll.html#method.poll
/// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
/// [readable]: struct.Ready.html#associatedconstant.READABLE
/// [writable]: struct.Ready.html#associatedconstant.WRITABLE
/// [error]: struct.Ready.html#associatedconstant.ERROR
/// [timer]: struct.Ready.html#associatedconstant.TIMER
/// [hup]: struct.Ready.html#associatedconstant.HUP
///
/// # Implementation notes
///
/// `Poll` is backed by the selector provided by the operating system.
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
/// `Poll` via [`EventedFd`].
///
/// On all platforms a call to [`Poll.poll`] is mostly just a direct call to the
/// system selector, but it also adds user space and timer events. Notifications
/// generated by user space registration ([`Registration`] and [`Notifier`]) are
/// handled by an internal readiness queue. Deadlines and timers use the same
/// queue. A single call to [`Poll.poll`] will collect events from both from the
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
pub struct Poll {
    selector: sys::Selector,
    userspace_events: Vec<Event>,
    deadlines: BinaryHeap<ReverseOrder<Deadline>>,
}

/// A type to check if `Evented` handles are called via a `Poll` instance.
///
/// This struct is used in the [`Evented`] trait. Since it can only created from
/// within the poll module it forces all calls to `Evented` handles to go via a
/// `Poll` instance.
///
/// [`Evented`]: ../event/trait.Evented.html
#[derive(Debug)]
pub struct PollCalled(());

impl Poll {
    /// Return a new `Poll` handle.
    ///
    /// This function will make a syscall to the operating system to create the
    /// system selector. If this syscall fails, `Poll::new` will return with the
    /// error.
    ///
    /// See [struct] level docs for more details.
    ///
    /// [struct]: struct.Poll.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::event::Events;
    /// use mio_st::poll::Poll;
    ///
    /// let mut poll = Poll::new()?;
    ///
    /// // Create a structure to receive polled events
    /// let mut events = Events::with_capacity(512, 512);
    ///
    /// // Wait for events, but none will be received because no `Evented`
    /// // handles have been registered with this `Poll` instance.
    /// poll.poll(&mut events, Some(Duration::from_millis(500)))?;
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn new() -> io::Result<Poll> {
        Ok(Poll {
            selector: sys::Selector::new()?,
            // TODO: try to get rid of this allocation.
            userspace_events: Vec::with_capacity(512),
            deadlines: BinaryHeap::new(),
        })
    }

    /// Register an `Evented` handle with the `Poll` instance.
    ///
    /// Once registered, the `Poll` instance will monitor the [`Evented`] handle
    /// for readiness state changes. When it notices a state change, it will
    /// return a readiness event for the handle the next time [`poll`] is
    /// called.
    ///
    /// See the [`struct`] docs for a high level overview.
    ///
    /// # Arguments
    ///
    /// `handle`: This is the handle that the `Poll` instance should monitor for
    /// readiness state changes.
    ///
    /// `id`: The caller picks a id to associate with the handle. When [`poll`]
    /// returns an event for the handle, this id is included. This allows the
    /// caller to map the event to its handle. The id associated with the
    /// `Evented` handle can be changed at any time by calling [`reregister`].
    /// Note that `id` may not be invalid, see [`is_valid`], and will return an
    /// error if it is.
    ///
    /// `interests`: Specifies which operations `Poll` should monitor for
    /// readiness. `Poll` will only return readiness events for operations
    /// specified by this argument. If a socket is registered with [readable]
    /// interests and the socket becomes writable, no event will be returned
    /// from [`poll`]. The readiness interests for an `Evented` handle can be
    /// changed at any time by calling [`reregister`]. Note that [timer]
    /// readiness events will always be triggered, even if the `Evented` handle
    /// is not registered with that interest. If an empty interests is passed
    /// this will return an error.
    ///
    /// `opt`: Specifies the registration option. Just like the interests, the
    /// option can be changed for an `Evented` handle at any time by calling
    /// [`reregister`].
    ///
    /// # Notes
    ///
    /// Unless otherwise specified, the caller should assume that once an
    /// `Evented` handle is registered with a `Poll` instance, it is bound to
    /// that `Poll` instance for the lifetime of the `Evented` handle. This
    /// remains true even if the `Evented` handle is deregistered from the poll
    /// instance using [`deregister`].
    ///
    /// [`Evented`]: ../event/trait.Evented.html
    /// [`poll`]: #method.poll
    /// [`struct`]: #
    /// [`reregister`]: #method.reregister
    /// [`is_valid`]: ../event/struct.EventedId.html#method.is_valid
    /// [readable]: struct.Ready.html#associatedconstant.READABLE
    /// [timer]: struct.Ready.html#associatedconstant.TIMER
    /// [`deregister`]: #method.deregister
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use std::time::{Duration, Instant};
    ///
    /// use mio_st::event::{Events, EventedId};
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Poll, Ready, PollOpt};
    ///
    /// // Create a new `Poll` instance as well a containers for the vents.
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(128, 128);
    ///
    /// // Create a TCP connection.
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `poll`.
    /// poll.register(&mut stream, EventedId(0), Ready::READABLE | Ready::WRITABLE, PollOpt::Edge)?;
    ///
    /// // Add a timeout so we don't wait too long for the connection to setup.
    /// poll.add_timeout(EventedId(0), Duration::from_millis(500));
    ///
    /// // Start the event loop.
    /// loop {
    ///     poll.poll(&mut events, None)?;
    ///
    ///     for event in &mut events {
    ///         if event.id() == EventedId(0) {
    ///             if event.readiness().is_timer() {
    ///                 // Connection likely timed out.
    ///             } else {
    ///                 // Connection is (probably) connected.
    ///             }
    ///             # return Ok(());
    ///         }
    ///     }
    /// }
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn register<E>(&mut self, handle: &mut E, id: EventedId, interests: Ready, opt: PollOpt) -> io::Result<()>
        where E: Evented + ?Sized
    {
        validate_args(id, interests)?;
        trace!("registering with poller: id: {:?}, interests: {:?}, opt: {:?}", id, interests, opt);
        handle.register(self, id, interests, opt, PollCalled(()))
    }

    /// Re-register an `Evented` handle with the `Poll` instance.
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
    /// instance of `Poll` otherwise the call to `reregister` will return with
    /// an error.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments and see the [`struct`] docs for a high level overview of
    /// polling.
    ///
    /// [readable]: struct.Ready.html#associatedconstant.READABLE
    /// [writable]: struct.Ready.html#associatedconstant.WRITABLE
    /// [`register`]: #method.register
    /// [`struct`]: #
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::event::EventedId;
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Poll, Ready, PollOpt};
    ///
    /// let mut poll = Poll::new()?;
    ///
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `Poll`, only with readable interest.
    /// poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOpt::Edge)?;
    ///
    /// // Reregister the connection specifying a different id and write interest
    /// // instead. `PollOpt::Edge` must be specified even though that value
    /// // is not being changed.
    /// poll.reregister(&mut stream, EventedId(2), Ready::WRITABLE, PollOpt::Edge)?;
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn reregister<E>(&mut self, handle: &mut E, id: EventedId, interests: Ready, opt: PollOpt) -> io::Result<()>
        where E: Evented + ?Sized
    {
        validate_args(id, interests)?;
        trace!("reregistering with poller: id: {:?}, interests: {:?}, opt: {:?}", id, interests, opt);
        handle.reregister(self, id, interests, opt, PollCalled(()))
    }

    /// Deregister an `Evented` handle with the `Poll` instance.
    ///
    /// When an `Evented` handle is deregistered, the `Poll` instance will no
    /// longer monitor it for readiness state changes. Unlike disabling handles
    /// with [`oneshot`], deregistering clears up any internal resources needed
    /// to track the handle.
    ///
    /// A handle can be passed back to `register` after it has been
    /// deregistered; however, it must be passed back to the **same** `Poll`
    /// instance.
    ///
    /// # Notes
    ///
    /// Calling `reregister` after `deregister` may be work on some platforms
    /// but not all. To properly re-register a handle after deregistering use
    /// `register`, this works on all platforms.
    ///
    /// [`oneshot`]: enum.PollOpt.html#variant.Oneshot
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::event::{Events, EventedId};
    /// use mio_st::net::TcpStream;
    /// use mio_st::poll::{Poll, Ready, PollOpt};
    ///
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(128, 128);
    ///
    /// let address = "216.58.193.100:80".parse()?;
    /// let mut stream = TcpStream::connect(address)?;
    ///
    /// // Register the connection with `Poll`.
    /// poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOpt::Edge)?;
    ///
    /// // Do stuff with the connection etc.
    ///
    /// // Deregister it so the resources can be cleaned up.
    /// poll.deregister(&mut stream)?;
    ///
    /// // Set a timeout because this poll should never receive any events.
    /// poll.poll(&mut events, Some(Duration::from_secs(1)))?;
    /// assert!(events.is_empty());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn deregister<E>(&mut self, handle: &mut E) -> io::Result<()>
        where E: Evented + ?Sized
    {
        trace!("deregistering handle with poller");
        handle.deregister(self, PollCalled(()))
    }

    /// Poll for readiness events.
    ///
    /// Blocks the current thread and waits for readiness events for any of the
    /// `Evented` handles that have been registered with this `Poll` instance
    /// previously.
    ///
    /// The function will block until either;
    ///
    /// * at least one readiness event has been received,
    /// * a timer was trigger, or
    /// * the `timeout` has elapsed.
    ///
    /// A `timeout` of `None` means that `poll` will block until one of the
    /// other two conditions are true. Note that the `timeout` will be rounded
    /// up to the system clock granularity (usually 1ms), and kernel scheduling
    /// delays mean that the blocking interval may be overrun by a small amount.
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
    /// [readable] set and one with [writable] set.
    ///
    /// See the [struct] level documentation for a higher level discussion of
    /// polling.
    ///
    /// [readable]: struct.Ready.html#associatedconstant.READABLE
    /// [writable]: struct.Ready.html#associatedconstant.WRITABLE
    /// [struct]: #
    pub fn poll(&mut self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        trace!("polling");
        let mut timeout = self.determine_timeout(timeout);

        // Clear any previously set events.
        events.reset();
        loop {
            let start = Instant::now();
            // Get the selector events.
            match self.selector.select(events.system_events_mut(), timeout) {
                Ok(()) => break,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                    // Interrupted by a signal; update timeout if necessary and
                    // retry.
                    if let Some(to) = timeout {
                        let elapsed = start.elapsed();
                        if elapsed >= to {
                            break;
                        } else {
                            timeout = Some(to - elapsed);
                        }
                    }
                },
                Err(e) => return Err(e),
            }
        }

        // Then poll deadlines.
        self.poll_deadlines(events);
        // Poll user space events.
        self.poll_userspace(events);
        Ok(())
    }

    /// Compute the timeout value passed to the system selector. If the
    /// user space queue has pending events, we still want to poll the system
    /// selector for new events, but we don't want to block the thread to wait
    /// for new events.
    ///
    /// If we have any deadlines the first one will also cap the timeout.
    fn determine_timeout(&mut self, timeout: Option<Duration>) -> Option<Duration> {
        if !self.userspace_events.is_empty() {
            // Userspace queue has events, so no blocking.
            return Some(Duration::from_millis(0));
        } else if let Some(deadline) = self.deadlines.peek() {
            let now = Instant::now();
            if deadline.deadline <= now {
                // Deadline has already expired, so no blocking.
                return Some(Duration::from_millis(0));
            }

            // Determine the timeout for the next deadline.
            let deadline_timeout = deadline.deadline.duration_since(now);
            match timeout {
                // The provided timeout is before the deadline timeout, so we'll
                // keep the original timeout.
                Some(timeout) if timeout < deadline_timeout => {},
                // Deadline timeout is sooner, use that.
                _ => return Some(deadline_timeout),
            }
        }

        timeout
    }

    /// Add a new user space event to the queue.
    pub(crate) fn userspace_add_event(&mut self, event: Event) {
        trace!("adding user space event: {:?}", event);
        self.userspace_events.push(event)
    }

    /// Poll user space events.
    fn poll_userspace(&mut self, events: &mut Events) {
        let n_copied = events.extend_events(&self.userspace_events);
        let left = self.userspace_events.len() - n_copied;
        if left == 0 {
            unsafe { self.userspace_events.set_len(0); }
        } else {
            // Move all leftover elements to the beginning of the vector.
            unsafe {
                let src = self.userspace_events.as_ptr().offset(n_copied as isize);
                ptr::copy(src, self.userspace_events.as_mut_ptr(), left);
                self.userspace_events.set_len(left);
            }
        }
    }

    /// Add a new deadline to Poll.
    ///
    /// This will create a new timer that will trigger an [`Event`] after the
    /// `deadline` has passed, which gets returned when [polling]. The `Event`
    /// will always have [`TIMER`] as `Ready` value and the same `id` as
    /// provided.
    ///
    /// [`Event`]: ../event/struct.Event.html
    /// [polling]: #method.poll
    /// [`TIMER`]: struct.Ready.html#associatedconstant.TIMER
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use std::time::Duration;
    ///
    /// use mio_st::poll::{Poll, Ready, PollOpt};
    /// use mio_st::event::{Event, Events, EventedId};
    ///
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(8, 8);
    ///
    /// // Add our timeout, this is shorthand for `Instant::now() + timeout`.
    /// poll.add_timeout(EventedId(0), Duration::from_millis(10));
    ///
    /// // Eventhough we don't provide a timeout to poll this will return in
    /// // roughly 10 milliseconds and return an event with our deadline.
    /// poll.poll(&mut events, None)?;
    ///
    /// for event in &mut events {
    ///     assert_eq!(event, Event::new(EventedId(0), Ready::TIMER));
    /// }
    /// #   Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn add_deadline(&mut self, id: EventedId, deadline: Instant) -> io::Result<()> {
        trace!("adding deadline: id: {:?}, deadline: {:?}", id, deadline);
        validate_args(id, Ready::TIMER)
            .map(|()| self.deadlines.push(ReverseOrder(Deadline { id, deadline })))
    }

    /// Add a new timeout to Poll.
    ///
    /// This is a shorthand for `poll.add_deadline(id, Instant::now() +
    /// timeout)`, see [`add_deadline`].
    ///
    /// [`add_deadline`]: #method.add_deadline
    pub fn add_timeout(&mut self, id: EventedId, timeout: Duration) -> io::Result<()> {
        self.add_deadline(id, Instant::now() + timeout)
    }

    /// Remove a previously added deadline.
    ///
    /// It tries removes the deadline from Poll and returns it, if it hasn't
    /// fired yet.
    ///
    /// # Note
    ///
    /// This function is not at all good for performance. If your code can deal
    /// with timeouts firing after they're no longer needed, then you should not
    /// use this function and let the timeout be fired and simply ignore it.
    pub fn remove_deadline(&mut self, id: EventedId) -> Option<Instant> {
        trace!("removing deadline: id: {:?}", id);
        // TODO: optimize this.
        let index = self.deadlines.iter()
            .position(|deadline| deadline.id == id);

        if let Some(index) = index {
            let deadlines = mem::replace(&mut self.deadlines, BinaryHeap::new());
            let mut deadlines_vec = deadlines.into_vec();
            debug_assert_eq!(deadlines_vec[index].id, id,
                             "remove_deadline: removing an incorrect deadline");
            let deadline = deadlines_vec.remove(index);
            drop(mem::replace(&mut self.deadlines, BinaryHeap::from(deadlines_vec)));
            Some(deadline.deadline)
        } else {
            None
        }
    }

    /// Add expired deadlines to the the provided `events`.
    fn poll_deadlines(&mut self, events: &mut Events) {
        let now = Instant::now();

        for _ in 0..events.user_capacity_left() {
            match self.deadlines.peek().cloned() {
                Some(deadline) if deadline.deadline <= now => {
                    let deadline = self.deadlines.pop().unwrap();
                    let r = events.push(Event::new(deadline.id, Ready::TIMER));
                    debug_assert!(r, "tried to expand events");
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

/// Validate the provided arguments making sure the `id` is valid and the other
/// arguments aren't empty.
fn validate_args(id: EventedId, interests: Ready) -> io::Result<()> {
    if interests.is_empty() {
        Err(io::Error::new(io::ErrorKind::Other, "empty interests"))
    } else if !id.is_valid() {
        Err(io::Error::new(io::ErrorKind::Other, "invalid evented id"))
    } else {
        Ok(())
    }
}

/// A deadline in `Poll`.
///
/// This must be ordered by `deadline`, then `id`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Deadline {
    deadline: Instant,
    id: EventedId,
}

/// Reverses the order of the comparing arguments.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ReverseOrder<T>(T);

impl<T: Ord> Ord for ReverseOrder<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

impl<T: PartialOrd> PartialOrd for ReverseOrder<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.0.partial_cmp(&self.0)
    }
}

impl<T> ops::Deref for ReverseOrder<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
