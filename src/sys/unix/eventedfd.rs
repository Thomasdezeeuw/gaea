use std::io;
use std::os::unix::io::RawFd;

use crate::event::EventedId;
use crate::os::{Evented, Interests, PollOption, OsQueue};

/// Adapter for a `RawFd` providing an [`Evented`] implementation.
///
/// `EventedFd` enables registering any type with an file descriptor with
/// [`OsQueue`].
///
/// While only implementations for TCP and UDP are provided, Mio supports
/// registering any file descriptor that can be registered with the underlying
/// OS selector. `EventedFd` provides the necessary bridge.
///
/// Note that `EventedFd` takes a reference to a `RawFd`. This is because
/// `EventedFd` **does not** take ownership of the file descriptor.
/// Specifically, it will not manage any lifecycle related operations, such as
/// closing the file descriptor on drop. It is expected that the `EventedFd` is
/// constructed right before a call to [`OsQueue.register`]. See the examples
/// below for more detail.
///
/// For a owned, or managed, type see [`EventedIo`].
///
/// [`Evented`]: ../event/trait.Evented.html
/// [`OsQueue`]: ../poll/struct.OsQueue.html
/// [`OsQueue.register`]: ../poll/struct.OsQueue.html#method.register
/// [`EventedIo`]: struct.EventedIo.html
///
/// # Deregistering
///
/// The file descriptor doesn't need to be deregistered **iff** the file
/// descriptor is unique (i.e. it is not duplicated via `dup(2)`) and will be
/// deregistered when it is `close`d.
///
/// # Examples
///
/// Basic usage
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::net::TcpListener;
/// use std::os::unix::io::AsRawFd;
///
/// use mio_st::event::EventedId;
/// use mio_st::poll::{Interests, PollOption, OsQueue};
/// use mio_st::unix::EventedFd;
///
/// // Bind a listener from the standard library.
/// let listener = TcpListener::bind("127.0.0.1:0")?;
///
/// let mut poller = OsQueue::new()?;
///
/// // Register the listener using `EventedFd`.
/// poller.register(&mut EventedFd(&listener.as_raw_fd()), EventedId(0), Interests::READABLE, PollOption::Edge)?;
/// #     Ok(())
/// # }
/// ```
///
/// Implementing `Evented` for a custom type backed by a `RawFd`.
///
/// ```
/// use std::io;
/// use std::os::unix::io::RawFd;
///
/// use mio_st::event::{Evented, EventedId};
/// use mio_st::poll::{Interests, PollOption, OsQueue};
/// use mio_st::unix::EventedFd;
///
/// # #[allow(dead_code)]
/// pub struct MyIo {
///     fd: RawFd,
/// }
///
/// impl Evented for MyIo {
///     fn register(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         EventedFd(&self.fd).register(poller, id, interests, opt)
///     }
///
///     fn reregister(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         EventedFd(&self.fd).reregister(poller, id, interests, opt)
///     }
///
///     fn deregister(&mut self, poller: &mut OsQueue) -> io::Result<()> {
///         EventedFd(&self.fd).deregister(poller)
///     }
/// }
/// ```
#[derive(Debug)]
pub struct EventedFd<'a>(pub &'a RawFd);

impl<'a> Evented for EventedFd<'a> {
    fn register(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        poller.selector().register(*self.0, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        poller.selector().reregister(*self.0, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut OsQueue) -> io::Result<()> {
        poller.selector().deregister(*self.0)
    }
}
