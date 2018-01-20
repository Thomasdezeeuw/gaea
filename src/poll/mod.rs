use std::{io, mem, ops};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use std::collections::BinaryHeap;

use sys;
use event::{Event, Events, Evented};

mod opt;
mod ready;
mod token;

pub use self::opt::PollOpt;
pub use self::ready::Ready;
pub use self::token::Token;
pub(crate) use self::token::INVALID_TOKEN;

// TODO: update below to document that `deadlines` queue system.

// Poll is backed by two readiness queues. The first is a system readiness queue
// represented by `sys::Selector`. The system readiness queue handles events
// provided by the system, such as TCP and UDP. The second readiness queue is
// implemented in user space by `ReadinessQueue`. It provides a way to implement
// purely user space `Evented` types.
//
// `ReadinessQueue` is backed by a MPSC queue that supports reuse of linked
// list nodes. This significantly reduces the number of required allocations.
// Each `Registration` / `SetReadiness` pair allocates a single readiness node
// that is used for the lifetime of the registration.
//
// The readiness node also includes a single atomic variable, `state` that
// tracks most of the state associated with the registration. This includes the
// current readiness, interest, poll options, and internal state. When the node
// state is mutated, it is queued in the MPSC channel. A call to
// `ReadinessQueue::poll` will dequeue and process nodes. The node state can
// still be mutated while it is queued in the channel for processing.
// Intermediate state values do not matter as long as the final state is
// included in the call to `poll`. This is the eventually consistent nature of
// the readiness queue.
//
// The readiness node is ref counted using the `ref_count` field. On creation,
// the ref_count is initialized to 3: one `Registration` handle, one
// `SetReadiness` handle, and one for the readiness queue. Since the readiness queue
// doesn't *always* hold a handle to the node, we don't use the Arc type for
// managing ref counts (this is to avoid constantly incrementing and
// decrementing the ref count when pushing & popping from the queue). When the
// `Registration` handle is dropped, the `dropped` flag is set on the node, then
// the node is pushed into the registration queue. When Poll::poll pops the
// node, it sees the drop flag is set, and decrements it's ref count.
//
// The MPSC queue is a modified version of the intrusive MPSC node based queue
// described by 1024cores [1].
//
// The first modification is that two markers are used instead of a single
// `stub`. The second marker is a `sleep_marker` which is used to signal to
// producers that the consumer is going to sleep. This sleep_marker is only used
// when the queue is empty, implying that the only node in the queue is
// `end_marker`.
//
// The second modification is an `until` argument passed to the dequeue
// function. When `poll` encounters a level-triggered node, the node will be
// immediately pushed back into the queue. In order to avoid an infinite loop,
// `poll` before pushing the node, the pointer is saved off and then passed
// again as the `until` argument. If the next node to pop is `until`, then
// `Dequeue::Empty` is returned.
//
// [1] http://www.1024cores.net/home/lock-free-algorithms/queues/intrusive-mpsc-node-based-queue


/// Polls for readiness events on all registered values.
///
/// `Poll` allows a program to monitor a large number of `Evented` types,
/// waiting until one or more become "ready" for some class of operations; e.g.
/// reading and writing. An `Evented` type is considered ready if it is possible
/// to immediately perform a corresponding operation; e.g. [`read`] or
/// [`write`].
///
/// To use `Poll`, an `Evented` type must first be registered with the `Poll`
/// instance using the [`register`] method, supplying readiness interest. The
/// readiness interest tells `Poll` which specific operations on the handle to
/// monitor for readiness. A `Token` is also passed to the [`register`]
/// function. When `Poll` returns a readiness event, it will include this token.
/// This associates the event with the `Evented` handle that generated the
/// event.
///
/// [`read`]: tcp/struct.TcpStream.html#method.read
/// [`write`]: tcp/struct.TcpStream.html#method.write
/// [`register`]: #method.register
///
/// # Examples
///
/// A basic example -- establishing a `TcpStream` connection.
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Poll, Ready, PollOpt, Token};
/// use mio::net::TcpStream;
///
/// use std::net::{TcpListener, SocketAddr};
///
/// // Bind a server socket to connect to.
/// let addr: SocketAddr = "127.0.0.1:0".parse()?;
/// let mut server = TcpListener::bind(addr)?;
///
/// // Construct a new `Poll` handle as well as the `Events` we'll store into
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(1024);
///
/// // Connect the stream
/// let mut stream = TcpStream::connect(server.local_addr()?)?;
///
/// // Register the stream with `Poll`
/// poll.register(&mut stream, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
///
/// // Wait for the socket to become ready. This has to happens in a loop to
/// // handle spurious wakeups.
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in &mut events {
///         if event.token() == Token(0) && event.readiness().is_writable() {
///             // The socket connected (probably, it could still be a spurious
///             // wakeup)
///             return Ok(());
///         }
///     }
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// # Portability
///
/// Using `Poll` provides a portable interface across supported platforms as
/// long as the caller takes the following into consideration:
///
/// ### Spurious events
///
/// [`Poll::poll`] may return readiness events even if the associated
/// [`Evented`] handle is not actually ready. Given the same code, this may
/// happen more on some platforms than others. It is important to never assume
/// that, just because a readiness notification was received, that the
/// associated operation will as well.
///
/// If operation fails with [`WouldBlock`], then the caller should not treat
/// this as an error and wait until another readiness event is received.
///
/// ### Draining readiness
///
/// When using edge-triggered mode, once a readiness event is received, the
/// corresponding operation must be performed repeatedly until it returns
/// [`WouldBlock`]. Unless this is done, there is no guarantee that another
/// readiness event will be delivered, even if further data is received for the
/// [`Evented`] handle.
///
/// For example, in the first scenario described above, after step 5, even if
/// the socket receives more data there is no guarantee that another readiness
/// event will be delivered.
///
/// ### Readiness operations
///
/// The only readiness operations that are guaranteed to be present on all
/// supported platforms are [`readable`] and [`writable`]. All other readiness
/// operations may have false negatives and as such should be considered
/// **hints**. This means that if a socket is registered with [`readable`],
/// [`error`], and [`hup`] interest, and either an error or hup is received, a
/// readiness event will be generated for the socket, but it **may** only
/// include `readable` readiness. Also note that, given the potential for
/// spurious events, receiving a readiness event with `hup` or `error` doesn't
/// actually mean that a `read` on the socket will return a result matching the
/// readiness event.
///
/// In other words, portable programs that explicitly check for [`hup`] or
/// [`error`] readiness should be doing so as an **optimization** and always be
/// able to handle an error or HUP situation when performing the actual read
/// operation.
///
/// [`readable`]: struct.Ready.html#method.readable
/// [`writable`]: struct.Ready.html#method.writable
/// [`error`]: struct.Ready.html#associatedconstant.ERROR
/// [`hup`]: struct.Ready.html#associatedconstant.HUP
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
/// use mio::{Poll, Ready, PollOpt, Token};
/// use mio::net::TcpStream;
/// use std::time::Duration;
/// use std::thread;
///
/// let mut sock = TcpStream::connect("216.58.193.100:80".parse()?)?;
///
/// thread::sleep(Duration::from_secs(1));
///
/// let mut poll = Poll::new()?;
///
/// // The connect is not guaranteed to have started until it is registered at
/// // this point
/// poll.register(&mut sock, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// # Implementation notes
///
/// `Poll` is backed by the selector provided by the operating system.
///
/// |      OS    |  Selector |
/// |------------|-----------|
/// | Linux      | [epoll]   |
/// | OS X, iOS  | [kqueue]  |
/// | Windows    | [IOCP]    |
/// | FreeBSD    | [kqueue]  |
/// | Android    | [epoll]   |
///
/// On all supported platforms, socket operations are handled by using the
/// system selector. Platform specific extensions (e.g. [`EventedFd`]) allow
/// accessing other features provided by individual system selectors. For
/// example, Linux's [`signalfd`] feature can be used by registering the FD with
/// `Poll` via [`EventedFd`].
///
/// On all platforms except windows, a call to [`Poll::poll`] is mostly just a
/// direct call to the system selector. However, [IOCP] uses a completion model
/// instead of a readiness model. In this case, `Poll` must adapt the completion
/// model Mio's API. While non-trivial, the bridge layer is still quite
/// efficient. The most expensive part being calls to `read` and `write` require
/// data to be copied into an intermediate buffer before it is passed to the
/// kernel.
///
/// Notifications generated by [`SetReadiness`] are handled by an internal
/// readiness queue. A single call to [`Poll::poll`] will collect events from
/// both from the system selector and the internal readiness queue.
///
/// [epoll]: http://man7.org/linux/man-pages/man7/epoll.7.html
/// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
/// [IOCP]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa365198(v=vs.85).aspx
/// [`signalfd`]: http://man7.org/linux/man-pages/man2/signalfd.2.html
/// [`EventedFd`]: unix/struct.EventedFd.html
/// [`SetReadiness`]: struct.SetReadiness.html
/// [`Poll::poll`]: struct.Poll.html#method.poll
#[derive(Debug)]
pub struct Poll {
    selector: sys::Selector,
    userspace_events: Vec<Event>,
    deadlines: BinaryHeap<ReverseOrder<Deadline>>,
}

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
    /// use mio::{Poll, Events};
    /// use std::time::Duration;
    ///
    /// let mut poll = Poll::new()?;
    ///
    /// // Create a structure to receive polled events
    /// let mut events = Events::with_capacity(1024);
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
            deadlines: BinaryHeap::new(),
            userspace_events: Vec::new(),
        })
    }

    /// Register an `Evented` handle with the `Poll` instance.
    ///
    /// Once registered, the `Poll` instance will monitor the `Evented` handle
    /// for readiness state changes. When it notices a state change, it will
    /// return a readiness event for the handle the next time [`poll`] is
    /// called.
    ///
    /// See the [`struct`] docs for a high level overview.
    ///
    /// # Arguments
    ///
    /// `handle: &E: Evented`: This is the handle that the `Poll` instance
    /// should monitor for readiness state changes.
    ///
    /// `token: Token`: The caller picks a token to associate with the socket.
    /// When [`poll`] returns an event for the handle, this token is included.
    /// This allows the caller to map the event to its handle. The token
    /// associated with the `Evented` handle can be changed at any time by
    /// calling [`reregister`].
    ///
    /// `token` cannot be `Token(usize::MAX)` as it is reserved for internal
    /// usage.
    ///
    /// See documentation on [`Token`] for an example showing how to pick
    /// [`Token`] values.
    ///
    /// `interest: Ready`: Specifies which operations `Poll` should monitor for
    /// readiness. `Poll` will only return readiness events for operations
    /// specified by this argument.
    ///
    /// If a socket is registered with [`readable`] interest and the socket
    /// becomes writable, no event will be returned from [`poll`].
    ///
    /// The readiness interest for an `Evented` handle can be changed at any
    /// time by calling [`reregister`].
    ///
    /// `opts: PollOpt`: Specifies the registration options. The most common
    /// options being [`level`] for level-triggered events, [`edge`] for
    /// edge-triggered events, and [`oneshot`].
    ///
    /// The registration options for an `Evented` handle can be changed at any
    /// time by calling [`reregister`].
    ///
    /// # Notes
    ///
    /// Unless otherwise specified, the caller should assume that once an
    /// `Evented` handle is registered with a `Poll` instance, it is bound to
    /// that `Poll` instance for the lifetime of the `Evented` handle. This
    /// remains true even if the `Evented` handle is deregistered from the poll
    /// instance using [`deregister`].
    ///
    /// # Undefined behaviour
    ///
    /// Reusing a token with a different `Evented` without deregistering (or
    /// closing) the original `Evented` will result in undefined behaviour.
    ///
    /// [`struct`]: #
    /// [`reregister`]: #method.reregister
    /// [`deregister`]: #method.deregister
    /// [`poll`]: #method.poll
    /// [`level`]: struct.PollOpt.html#method.level
    /// [`edge`]: struct.PollOpt.html#method.edge
    /// [`oneshot`]: struct.PollOpt.html#method.oneshot
    /// [`Token`]: struct.Token.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    /// use std::time::{Duration, Instant};
    ///
    /// let mut poll = Poll::new()?;
    /// let mut socket = TcpStream::connect("216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.register(&mut socket, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// let start = Instant::now();
    /// let timeout = Duration::from_millis(500);
    ///
    /// loop {
    ///     let elapsed = start.elapsed();
    ///
    ///     if elapsed >= timeout {
    ///         // Connection timed out
    ///         return Ok(());
    ///     }
    ///
    ///     let remaining = timeout - elapsed;
    ///     poll.poll(&mut events, Some(remaining))?;
    ///
    ///     for event in &mut events {
    ///         if event.token() == Token(0) {
    ///             // Something (probably) happened on the socket.
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn register<E>(&mut self, handle: &mut E, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()>
        where E: Evented + ?Sized
    {
        token.validate()?;
        trace!("registering with poller, token: {:?}, interest: {:?}, opts: {:?}", token, interest, opts);
        handle.register(self, token, interest, opts)
    }

    /// Re-register an `Evented` handle with the `Poll` instance.
    ///
    /// Re-registering an `Evented` handle allows changing the details of the
    /// registration. Specifically, it allows updating the associated `token`,
    /// `interest`, and `opts` specified in previous `register` and `reregister`
    /// calls.
    ///
    /// The `reregister` arguments fully override the previous values. In other
    /// words, if a socket is registered with [`readable`] interest and the call
    /// to `reregister` specifies [`writable`], then read interest is no longer
    /// requested for the handle.
    ///
    /// The `Evented` handle must have previously been registered with this
    /// instance of `Poll` otherwise the call to `reregister` will return with
    /// an error.
    ///
    /// `token` cannot be `Token(usize::MAX)` as it is reserved for internal
    /// usage.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments and see the [`struct`] docs for a high level overview of
    /// polling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    ///
    /// let mut poll = Poll::new()?;
    /// let mut socket = TcpStream::connect("216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`, requesting readable
    /// poll.register(&mut socket, Token(0), Ready::READABLE, PollOpt::EDGE)?;
    ///
    /// // Reregister the socket specifying a different token and write interest
    /// // instead. `PollOpt::EDGE` must be specified even though that value
    /// // is not being changed.
    /// poll.reregister(&mut socket, Token(2), Ready::WRITABLE, PollOpt::EDGE)?;
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`struct`]: #
    /// [`register`]: #method.register
    /// [`readable`]: struct.Ready.html#associatedconstant.READABLE
    /// [`writable`]: struct.Ready.html#associatedconstant.WRITABLE
    pub fn reregister<E>(&mut self, handle: &mut E, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()>
        where E: Evented + ?Sized
    {
        token.validate()?;
        trace!("reregistering with poller, token: {:?}, interest: {:?}, opts: {:?}", token, interest, opts);
        handle.reregister(self, token, interest, opts)
    }

    /// Deregister an `Evented` handle with the `Poll` instance.
    ///
    /// When an `Evented` handle is deregistered, the `Poll` instance will
    /// no longer monitor it for readiness state changes. Unlike disabling
    /// handles with [`oneshot`], deregistering clears up any internal resources
    /// needed to track the handle.
    ///
    /// A handle can be passed back to `register` after it has been
    /// deregistered; however, it must be passed back to the **same** `Poll`
    /// instance.
    ///
    /// `Evented` handles are automatically deregistered when they are dropped.
    /// It is common to never need to explicitly call `deregister`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let mut poll = Poll::new()?;
    /// let mut socket = TcpStream::connect("216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.register(&mut socket, Token(0), Ready::READABLE, PollOpt::EDGE)?;
    ///
    /// poll.deregister(&mut socket)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Set a timeout because this poll should never receive any events.
    /// poll.poll(&mut events, Some(Duration::from_secs(1)))?;
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
        handle.deregister(self)
    }

    /// Wait for readiness events
    ///
    /// Blocks the current thread and waits for readiness events for any of the
    /// `Evented` handles that have been registered with this `Poll` instance.
    /// The function will block until either at least one readiness event has
    /// been received or `timeout` has elapsed. A `timeout` of `None` means that
    /// `poll` will block until a readiness event has been received.
    ///
    /// The supplied `events` will be cleared and newly received readiness events
    /// will be pushed onto the end. At most `events.capacity()` events will be
    /// returned. If there are further pending readiness events, they will be
    /// returned on the next call to `poll`.
    ///
    /// A single call to `poll` may result in multiple readiness events being
    /// returned for a single `Evented` handle. For example, if a TCP socket
    /// becomes both readable and writable, it may be possible for a single
    /// readiness event to be returned with both [`readable`] and [`writable`]
    /// readiness **OR** two separate events may be returned, one with
    /// [`readable`] set and one with [`writable`] set.
    ///
    /// Note that the `timeout` will be rounded up to the system clock
    /// granularity (usually 1ms), and kernel scheduling delays mean that
    /// the blocking interval may be overrun by a small amount.
    ///
    /// `poll` returns the number of readiness events that have been pushed into
    /// `events` or `Err` when an error has been encountered with the system
    /// selector.  The value returned is deprecated and will be removed in 0.7.0.
    /// Accessing the events by index is also deprecated.  Events can be
    /// inserted by other events triggering, thus making sequential access
    /// problematic.  Use the iterator API instead.  See [`iter`].
    ///
    /// See the [struct] level documentation for a higher level discussion of
    /// polling.
    ///
    /// [`readable`]: struct.Ready.html#associatedconstant.READABLE
    /// [`writable`]: struct.Ready.html#associatedconstant.WRITABLE
    /// [struct]: #
    /// [`iter`]: struct.Events.html#method.iter
    ///
    /// # Examples
    ///
    /// A basic example -- establishing a `TcpStream` connection.
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    ///
    /// use std::net::{TcpListener, SocketAddr};
    /// use std::thread;
    ///
    /// // Bind a server socket to connect to.
    /// let addr: SocketAddr = "127.0.0.1:0".parse()?;
    /// let mut server = TcpListener::bind(addr)?;
    /// let addr = server.local_addr()?.clone();
    ///
    /// // Spawn a thread to accept the socket
    /// thread::spawn(move || {
    ///     let _ = server.accept();
    /// });
    ///
    /// // Construct a new `Poll` handle as well as the `Events` we'll store into
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Connect the stream
    /// let mut stream = TcpStream::connect(addr)?;
    ///
    /// // Register the stream with `Poll`
    /// poll.register(&mut stream, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
    ///
    /// // Wait for the socket to become ready. This has to happens in a loop to
    /// // handle spurious wakeups.
    /// loop {
    ///     poll.poll(&mut events, None)?;
    ///
    ///     for event in &mut events {
    ///         if event.token() == Token(0) && event.readiness().is_writable() {
    ///             // The socket connected (probably, it could still be a spurious
    ///             // wakeup)
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [struct]: #
    pub fn poll(&mut self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let mut timeout = self.determine_timeout(timeout);

        // Clear any previously set events.
        events.reset();
        loop {
            let start = Instant::now();
            // Get the selector events.
            match self.selector.select(events.inner_mut(), timeout) {
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

        // Poll user space events.
        self.poll_userspace(events);
        // Then poll deadlines.
        self.poll_deadlines(events);
        Ok(())
    }

    /// Compute the timeout value passed to the system selector. If the
    /// user space queue has pending events, we still want to poll the system
    /// selector for new events, but we don't want to block the thread to wait
    /// for new events.
    ///
    /// If we have any deadlines the first one will also cap the timeout.
    fn determine_timeout(&mut self, timeout: Option<Duration>) -> Option<Duration> {
        if self.userspace_events.len() > 0 {
            // Userspace queue has events, so no blocking.
            return Some(Duration::from_millis(0));
        } else if let Some(deadline) = self.deadlines.peek() {
            let now = Instant::now();
            if deadline.deadline < now {
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
        self.userspace_events.push(event)
    }

    /// Poll user space events.
    fn poll_userspace(&mut self, events: &mut Events) {
        events.extend_events(&self.userspace_events);
        self.userspace_events.clear();
    }

    /// Add a new deadline to Poll.
    ///
    /// This will create a new timer that will trigger an [`Event`] after the
    /// `deadline` has passed, which gets returned when [polling]. The `Event`
    /// will always have [`TIMER`] as `Ready` value and the same `token` as
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
    /// use mio::poll::{Poll, Token, Ready, PollOpt};
    /// use mio::event::{Event, Events};
    ///
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(128);
    ///
    /// // Add our timeout, this is shorthand for `Instant::now() + timeout`.
    /// poll.add_timeout(Token(0), Duration::from_millis(10));
    ///
    /// // Eventhough we don't provide a timeout to poll this will return in
    /// // roughly 10 milliseconds and return an event with our deadline.
    /// poll.poll(&mut events, None)?;
    ///
    /// for event in &mut events {
    ///     assert_eq!(event, Event::new(Ready::TIMER, Token(0)));
    /// }
    /// #   Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn add_deadline(&mut self, token: Token, deadline: Instant) {
        self.deadlines.push(ReverseOrder(Deadline { token, deadline }));
    }

    /// Add a new timeout to Poll.
    ///
    /// This is a shorthand for `poll.add_deadline(token, Instant::now() +
    /// timeout)`, see [`add_deadline`].
    ///
    /// [`add_deadline`]: #method.add_deadline
    pub fn add_timeout(&mut self, token: Token, timeout: Duration) {
        self.add_deadline(token, Instant::now() + timeout)
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
    /// use this function and let the timeout be fired and ignored.
    pub fn remove_deadline(&mut self, token: Token) -> Option<Instant> {
        // TODO: optimize this.
        let index = self.deadlines.iter()
            .position(|deadline| deadline.token == token);

        if let Some(index) = index {
            let deadlines = mem::replace(&mut self.deadlines, BinaryHeap::new());
            let mut deadlines_vec = deadlines.into_vec();
            debug_assert_eq!(deadlines_vec[index].token, token,
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
        loop {
            match self.deadlines.peek().cloned() {
                Some(deadline) if deadline.deadline <= now => {
                    let deadline = self.deadlines.pop().unwrap();
                    events.push(Event::new(Ready::TIMER, deadline.token));
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

/// A deadline in `Poll`.
///
/// This must be ordered by `deadline`, then `token`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Deadline {
    deadline: Instant,
    token: Token,
}

/// Reverses the order of the comparing arguments.
///
/// ```ignore
/// assert!(ReverseOrder(10) > ReverseOrder(100));
/// assert!(ReverseOrder(10) < ReverseOrder(1));
/// ```
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
