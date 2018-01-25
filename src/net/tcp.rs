use std::io::{self, Read, Write};
use std::net::{self, Shutdown, SocketAddr};
#[cfg(unix)]
use std::os::unix::io::{IntoRawFd, AsRawFd, FromRawFd, RawFd};
use std::time::Duration;

use net2::TcpBuilder;

use sys;
use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready, Private};

/// A non-blocking TCP stream between a local socket and a remote socket.
///
/// If fine-grained control over the creation of the socket is desired, you can
/// use `net2::TcpBuilder` to configure a socket and then pass its socket to
/// `TcpStream::connect_stream` to transfer ownership into mio and schedule the
/// connect operation.
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Events, EventedId};
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poll, PollOpt, Ready};
///
/// let address = "127.0.0.1:8888".parse()?;
/// let mut stream = TcpStream::connect(address)?;
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(128, 128);
///
/// // Register the socket with `Poll`.
/// poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOpt::Edge)?;
///
/// poll.poll(&mut events, None)?;
///
/// // The socket might be ready at this point.
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct TcpStream {
    inner: sys::TcpStream,
}

impl TcpStream {
    /// Create a new TCP stream and issue a non-blocking connect to the
    /// specified address.
    pub fn connect(addr: SocketAddr) -> io::Result<TcpStream> {
        let sock = match addr {
            SocketAddr::V4(..) => TcpBuilder::new_v4(),
            SocketAddr::V6(..) => TcpBuilder::new_v6(),
        }?;
        TcpStream::connect_stream(sock.to_tcp_stream()?, addr)
    }

    /// Creates a new `TcpStream` from the pending socket inside the given
    /// `std::net::TcpBuilder`, connecting it to the address specified.
    ///
    /// This constructor allows configuring the socket before it's actually
    /// connected, and this function will transfer ownership to the returned
    /// `TcpStream` if successful. An unconnected `TcpStream` can be created
    /// with the `net2::TcpBuilder` type (and also configured via that route).
    ///
    /// The platform specific behavior of this function looks like:
    ///
    /// * On Unix, the socket is placed into nonblocking mode and then a
    ///   `connect` call is issued.
    pub fn connect_stream(stream: net::TcpStream, addr: SocketAddr) -> io::Result<TcpStream> {
        sys::TcpStream::connect(stream, &addr).map(|inner| TcpStream { inner })
    }

    /// Creates a new `TcpStream` from a standard `net::TcpStream`.
    ///
    /// This function is intended to be used to wrap a TCP stream from the
    /// standard library in the mio equivalent. The conversion here will
    /// automatically set `stream` to nonblocking and the returned object should
    /// be ready to get associated with an event loop.
    ///
    /// Note that the TCP stream here will not have `connect` called on it, so
    /// it should already be connected via some other means (be it manually, the
    /// `net2` crate, or the standard library).
    pub fn from_std_stream(stream: net::TcpStream) -> io::Result<TcpStream> {
        sys::TcpStream::from_std_stream(stream).map(|inner| TcpStream { inner })
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    pub fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the socket address of the local half of this TCP connection.
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Sets whether keepalive messages are enabled to be sent on this socket.
    ///
    /// On Unix, this option will set the `SO_KEEPALIVE` as well as the
    /// `TCP_KEEPALIVE` or `TCP_KEEPIDLE` option (depending on your platform).
    ///
    /// If `None` is specified then keepalive messages are disabled, otherwise
    /// the duration specified will be the time to remain idle before sending a
    /// TCP keepalive probe.
    ///
    /// Some platforms specify this value in seconds, so sub-second
    /// specifications may be omitted.
    pub fn set_keepalive(&mut self, keepalive: Option<Duration>) -> io::Result<()> {
        self.inner.set_keepalive(keepalive)
    }

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the duration of time between them.
    ///
    /// For more information about this option, see [`set_keepalive`].
    ///
    /// [`set_keepalive`]: #method.set_keepalive
    pub fn keepalive(&mut self) -> io::Result<Option<Duration>> {
        self.inner.keepalive()
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying recv system call.
    pub fn peek(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value (see the
    /// documentation of [`Shutdown`]).
    ///
    /// [`Shutdown`]: https://doc.rust-lang.org/nightly/std/net/enum.Shutdown.html
    pub fn shutdown(&mut self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&mut self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Evented for TcpStream {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.inner.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.inner.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
        self.inner.deregister(poll, p)
    }
}

#[cfg(unix)]
impl IntoRawFd for TcpStream {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(unix)]
impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpStream {
        TcpStream {
            inner: FromRawFd::from_raw_fd(fd),
        }
    }
}

/// A structure representing a socket listener.
///
/// If fine-grained control over the binding and listening process for a socket
/// is desired then use the `net2::TcpBuilder` methods, in the [`net2`] crate.
/// This can be used in combination with the [`TcpListener::from_std_listener`]
/// method to transfer ownership into mio. Also see the second example below.
///
/// [`net2`]: https://crates.io/crates/net2
/// [`TcpListener::from_std_listener`]: #method.from_std_listener
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Events, EventedId};
/// use mio_st::net::TcpListener;
/// use mio_st::poll::{Poll, PollOpt, Ready};
///
/// let address = "127.0.0.1:7777".parse()?;
/// let mut listener = TcpListener::bind(address)?;
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(128, 128);
///
/// // Register the socket with `Poll`
/// poll.register(&mut listener, EventedId(0), Ready::WRITABLE, PollOpt::Edge)?;
///
/// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// // There may be a socket ready to be accepted.
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// Using the [`net2`] crate to create a listener and change settings on the
/// accepted incoming connection.
///
/// ```
/// extern crate net2;
///
/// # extern crate mio_st;
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
/// use std::net::SocketAddr;
///
/// use mio_st::net::{TcpListener, TcpStream};
/// use net2::{self, TcpStreamExt};
///
/// // Create a new `net2` `TcpBuilder`.
/// let builder = net2::TcpBuilder::new_v4()?;
///
/// // Bind the tcp socket and start listening, this will return a
/// // `std::net::TcpListener`.
/// let addr: SocketAddr = "127.0.0.1:12345".parse()?;
/// let std_listener = builder.bind(addr)?.listen(128)?;
///
/// // Convert the listener into an mio listener.
/// let mut mio_listener = TcpListener::from_std_listener(std_listener)?;
///
/// // Use `mio_listener` as normal, such as registering it with poll, etc.
///
/// // This may return a `WouldBlock` error.
/// # (|| -> ::std::io::Result<()> {
/// let (std_stream, addr) = mio_listener.accept_std()?;
///
/// // Use net2's `TcpStreamExt` trait to modifiy the receiving buffer size.
/// std_stream.set_recv_buffer_size(4 * 1024)?;
///
/// // Now convert the stream into a mio stream, set it non-blocking mode etc.
/// let mio_stream = TcpStream::from_std_stream(std_stream)?;
///
/// // Now the `mio_stream` can be used as normal.
/// # drop(mio_stream);
/// # Ok(())
/// # })();
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct TcpListener {
    inner: sys::TcpListener,
}

impl TcpListener {
    /// Convenience method to bind a new TCP listener to the specified address
    /// to receive new connections.
    ///
    /// This function will take the following steps:
    ///
    /// 1. Create a new TCP socket.
    /// 2. Set the `SO_REUSEADDR` option on the socket.
    /// 3. Bind the socket to the specified address.
    /// 4. Call `listen` on the socket to prepare it to receive new connections.
    pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
        let socket = match addr {
            SocketAddr::V4(..) => TcpBuilder::new_v4(),
            SocketAddr::V6(..) => TcpBuilder::new_v6(),
        }?;

        if cfg!(unix) {
            let _ = socket.reuse_address(true)?;
        }

        let listener = socket.bind(addr)?.listen(128)?;
        TcpListener::from_std_listener(listener)
    }

    /// Creates a new `TcpListener` from an instance of a
    /// `std::net::TcpListener` type.
    ///
    /// This function will set the `listener` provided into nonblocking mode,
    /// and otherwise the listener will just be wrapped up in an mio listener
    /// ready to accept new connections and become associated with `Poll`.
    pub fn from_std_listener(listener: net::TcpListener) -> io::Result<TcpListener> {
        sys::TcpListener::new(listener).map(|inner| TcpListener { inner })
    }

    /// Accepts a new `TcpStream`.
    ///
    /// This may return an [`WouldBlock`] error, this means a stream may be
    /// ready at a later point and one should wait for a notification before
    /// calling `accept` again.
    ///
    /// If an accepted stream is returned, the remote address of the peer is
    /// returned along with it.
    ///
    /// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
    pub fn accept(&mut self) -> io::Result<(TcpStream, SocketAddr)> {
        self.accept_std().and_then(|(stream, addr)| {
            let stream = TcpStream::from_std_stream(stream)?;
            Ok((stream, addr))
        })
    }

    /// Accepts a new `std::net::TcpStream`.
    ///
    /// This method is the same as `accept`, except that it returns a TCP socket
    /// *in blocking mode* which isn't bound to `mio`. This can be later then
    /// converted to a `mio` type, if necessary.
    pub fn accept_std(&mut self) -> io::Result<(net::TcpStream, SocketAddr)> {
        self.inner.accept()
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&mut self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl Evented for TcpListener {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.inner.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.inner.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
        self.inner.deregister(poll, p)
    }
}

#[cfg(unix)]
impl IntoRawFd for TcpListener {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(unix)]
impl FromRawFd for TcpListener {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpListener {
        TcpListener {
            inner: sys::TcpListener::from_raw_fd(fd),
        }
    }
}
