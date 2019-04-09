use std::io;

use crate::event;
use crate::os::{Interests, OsQueue, RegisterOption};

/// A handle that may be registered with [`OsQueue`].
///
/// Handles that implement `Evented` can be registered with [`OsQueue`]. The
/// methods on the trait **should not** be called directly, instead the
/// equivalent methods should be called on [`OsQueue`].
///
/// See [`OsQueue` documentation] for more details.
///
/// [`OsQueue` documentation]: OsQueue
///
/// # Implementing `Evented`
///
/// Implementations of `Evented` are always backed by *system* handles, which
/// are backed by sockets or other system handles. The `Evented` handles will be
/// monitored by the Operating System selector. In this case, an implementation
/// of `Evented` delegates to a lower level handle. Examples of this are
/// [`TcpStream`]s, or the *unix only* [`EventedFd`].
///
/// [`TcpStream`]: crate::net::TcpStream
/// [`EventedFd`]: crate::unix::EventedFd
///
/// # Dropping `Evented` types
///
/// All `Evented` types, unless otherwise specified, need to be deregistered
/// before being dropped for them to not leak resources. This goes against the
/// normal drop behaviour of types in Rust which cleanup after themselves, e.g.
/// a `File` will close itself. However since deregistering needs mutable access
/// to [`OsQueue`] this cannot be done while being dropped.
///
/// # Examples
///
/// Implementing `Evented` on a struct containing a system handle, such as a
/// [`TcpStream`].
///
/// ```
/// use std::io;
///
/// use mio_st::event;
/// use mio_st::net::TcpStream;
/// use mio_st::os::{Evented, Interests, RegisterOption, OsQueue};
///
/// # #[allow(dead_code)]
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
///         // Delegate the `register` call to `socket`.
///         self.socket.register(os_queue, id, interests, opt)
///     }
///
///     fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`.
///         self.socket.reregister(os_queue, id, interests, opt)
///     }
///
///     fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
///         // Delegate the `deregister` call to `socket`.
///         self.socket.deregister(os_queue)
///     }
/// }
/// ```
pub trait Evented {
    /// Register `self` with the given `OsQueue` instance.
    ///
    /// This function should not be called directly, use [`OsQueue.register`]
    /// instead.
    ///
    /// [`OsQueue.register`]: crate::os::OsQueue::register
    fn register(&mut self, selector: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()>;

    /// Reregister `self` with the given `OsQueue` instance.
    ///
    /// This function should not be called directly, use [`OsQueue.reregister`]
    /// instead.
    ///
    /// [`OsQueue.reregister`]: crate::os::OsQueue::reregister
    fn reregister(&mut self, selector: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()>;

    /// Deregister `self` from the given `OsQueue` instance
    ///
    /// This function should not be called directly, use [`OsQueue.deregister`]
    /// instead.
    ///
    /// [`OsQueue.deregister`]: crate::os::OsQueue::deregister
    fn deregister(&mut self, selector: &mut OsQueue) -> io::Result<()>;
}
