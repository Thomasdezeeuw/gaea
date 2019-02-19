use std::io;

use crate::event::EventedId;
use crate::os::{Interests, PollOption, OsQueue};

// TODO: docs and examples.

/// A value that may be registered with `OsQueue`.
///
/// Values that implement `Evented` can be registered with [`OsQueue`]. The
/// methods on the trait cannot be called directly, instead the equivalent
/// methods must be called on a [`OsQueue`] instance.
///
/// See [`OsQueue`] for more details.
///
/// [`OsQueue`]: ../poll/struct.OsQueue.html
///
/// # Implementing `Evented`
///
/// Implementation of `Evented` are always backed by **system** handles, which
/// are backed by sockets or other system handles. The `Evented` handles will be
/// monitored by the system selector. In this case, an implementation of
/// `Evented` delegates to a lower level handle. Examples of this are
/// [`TcpStream`]s, or the *unix only* [`EventedFd`].
///
/// [`TcpStream`]: ../net/struct.TcpStream.html
/// [`EventedFd`]: ../unix/struct.EventedFd.html
///
/// # Dropping `Evented` types
///
/// All `Evented` types, unless otherwise specified, need to be deregistered
/// before being dropped for them to not leak resources. This goes against the
/// normal drop behaviour of types in Rust which cleanup after themselves, e.g.
/// a `File` will close itself. However since deregistering needs mutable access
/// to `OsQueue` this cannot be done while being dropped.
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
/// use mio_st::poll::{Interests, PollOption, OsQueue};
///
/// # #[allow(dead_code)]
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, selector: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         // Delegate the `register` call to `socket`.
///         self.socket.register(selector, id, interests, opt)
///     }
///
///     fn reregister(&mut self, selector: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`.
///         self.socket.reregister(selector, id, interests, opt)
///     }
///
///     fn deregister(&mut self, selector: &mut OsQueue) -> io::Result<()> {
///         // Delegate the `deregister` call to `socket`.
///         self.socket.deregister(selector)
///     }
/// }
/// ```
pub trait Evented {
    /// Register `self` with the given `OsQueue` instance.
    ///
    /// This function should not be called directly, use [`OsQueue.register`]
    /// instead.
    ///
    /// [`OsQueue.register`]: ../poll/struct.OsQueue.html#method.register
    fn register(&mut self, selector: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>;

    /// Reregister `self` with the given `OsQueue` instance.
    ///
    /// This function should not be called directly, use [`OsQueue.reregister`]
    /// instead.
    ///
    /// [`OsQueue.reregister`]: ../poll/struct.OsQueue.html#method.reregister
    fn reregister(&mut self, selector: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>;

    /// Deregister `self` from the given `OsQueue` instance
    ///
    /// This function should not be called directly, use [`OsQueue.deregister`]
    /// instead.
    ///
    /// [`OsQueue.deregister`]: ../poll/struct.OsQueue.html#method.deregister
    fn deregister(&mut self, selector: &mut OsQueue) -> io::Result<()>;
}
