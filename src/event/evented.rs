use std::io;

use crate::event::EventedId;
use crate::poll::{Interests, PollOption, Poller};

/// A value that may be registered with `Poller`.
///
/// Values that implement `Evented` can be registered with [`Poller`]. The
/// methods on the trait cannot be called directly, instead the equivalent
/// methods must be called on a [`Poller`] instance.
///
/// See [`Poller`] for more details.
///
/// [`Poller`]: ../poll/struct.Poller.html
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
/// to `Poller` this cannot be done while being dropped.
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
/// use mio_st::poll::{Interests, PollOption, Poller};
///
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         // Delegate the `register` call to `socket`.
///         self.socket.register(poller, id, interests, opt)
///     }
///
///     fn reregister(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
///         // Delegate the `reregister` call to `socket`.
///         self.socket.reregister(poller, id, interests, opt)
///     }
///
///     fn deregister(&mut self, poller: &mut Poller) -> io::Result<()> {
///         // Delegate the `deregister` call to `socket`.
///         self.socket.deregister(poller)
///     }
/// }
/// ```
pub trait Evented {
    /// Register `self` with the given `Poller` instance.
    ///
    /// This function should not be called directly, use [`Poller.register`]
    /// instead.
    ///
    /// [`Poller.register`]: ../poll/struct.Poller.html#method.register
    fn register(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>;

    /// Reregister `self` with the given `Poller` instance.
    ///
    /// This function should not be called directly, use [`Poller.reregister`]
    /// instead.
    ///
    /// [`Poller.reregister`]: ../poll/struct.Poller.html#method.reregister
    fn reregister(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()>;

    /// Deregister `self` from the given `Poller` instance
    ///
    /// This function should not be called directly, use [`Poller.deregister`]
    /// instead.
    ///
    /// [`Poller.deregister`]: ../poll/struct.Poller.html#method.deregister
    fn deregister(&mut self, poller: &mut Poller) -> io::Result<()>;
}
