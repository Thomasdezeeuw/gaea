use std::io;
use std::os::unix::io::RawFd;

use crate::event;
use crate::os::{Evented, Interests, RegisterOption, OsQueue};

/// Adapter for a `RawFd` providing an [`Evented`] implementation.
///
/// `EventedFd` enables registering any type with an file descriptor with
/// [`OsQueue`].
///
/// While only implementations for TCP and UDP are provided, registering any
/// file descriptor, that can be registered with the underlying OS selector, can
/// be registered with `OsQueue`. `EventedFd` provides the necessary bridge.
///
/// Note that `EventedFd` takes a reference to a `RawFd`. This is because
/// `EventedFd` **does not** take ownership of the file descriptor.
/// Specifically, it will not manage any lifecycle related operations, such as
/// closing the file descriptor on drop. It is expected that the `EventedFd` is
/// constructed right before a call to [`OsQueue::register`]. See the examples
/// below for more detail.
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
/// use mio_st::event;
/// use mio_st::os::{Interests, RegisterOption, OsQueue};
/// use mio_st::unix::EventedFd;
///
/// // Bind a listener from the standard library.
/// let listener = TcpListener::bind("127.0.0.1:0")?;
///
/// let mut os_queue = OsQueue::new()?;
///
/// // Register the listener using `EventedFd`.
/// os_queue.register(&mut EventedFd(&listener.as_raw_fd()), event::Id(0), Interests::READABLE, RegisterOption::EDGE)?;
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
/// use mio_st::event;
/// use mio_st::os::{Evented, Interests, RegisterOption, OsQueue};
/// use mio_st::unix::EventedFd;
///
/// # #[allow(dead_code)]
/// pub struct MyIo {
///     fd: RawFd,
/// }
///
/// impl Evented for MyIo {
///     fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
///         EventedFd(&self.fd).register(os_queue, id, interests, opt)
///     }
///
///     fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
///         EventedFd(&self.fd).reregister(os_queue, id, interests, opt)
///     }
///
///     fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
///         EventedFd(&self.fd).deregister(os_queue)
///     }
/// }
/// ```
#[derive(Debug)]
pub struct EventedFd<'a>(pub &'a RawFd);

impl<'a> Evented for EventedFd<'a> {
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        os_queue.selector().register(*self.0, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        os_queue.selector().reregister(*self.0, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        os_queue.selector().deregister(*self.0)
    }
}
