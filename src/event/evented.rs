use std::io;

use poll::{Poll, PollCalled, PollOption};
use event::{EventedId, Ready};

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
/// use mio_st::event::{Evented, EventedId, Ready};
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poll, PollOption, PollCalled};
///
/// pub struct MyEvented {
///     /// Our system handle that implements `Evented`.
///     socket: TcpStream,
/// }
///
/// impl Evented for MyEvented {
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
///         // Delegate the `register` call to `socket`
///         self.socket.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
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
/// use mio_st::event::{Evented, EventedId, Ready};
/// use mio_st::poll::{Poll, PollOption, PollCalled};
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
///     fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
///         self.registration.register(poll, id, interests, opt, p)
///     }
///
///     fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
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
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()>;

    /// Reregister `self` with the given `Poll` instance.
    ///
    /// This function should not be called directly, use [`Poll.reregister`]
    /// instead.
    ///
    /// [`Poll.reregister`]: ../poll/struct.Poll.html#method.reregister
    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()>;

    /// Deregister `self` from the given `Poll` instance
    ///
    /// This function should not be called directly, use [`Poll.deregister`]
    /// instead.
    ///
    /// [`Poll.deregister`]: ../poll/struct.Poll.html#method.deregister
    fn deregister(&mut self, poll: &mut Poll, p: PollCalled) -> io::Result<()>;
}
