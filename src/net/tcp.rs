use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::sys;
use crate::event::{Evented, EventedId};
use crate::poll::{Interests, PollOption, Poller};

/// A non-blocking TCP stream between a local socket and a remote socket.
///
/// This works much like the `TcpStream` in the standard library, but the
/// [`Read`] and [`Write`] implementation don't block and instead return a
/// [`WouldBlock`] error.
///
/// [`Read`]: #impl-Read
/// [`Write`]: #impl-Write
/// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Events, EventedId};
/// use mio_st::net::TcpStream;
/// use mio_st::poll::{Poller, PollOption};
///
/// let address = "127.0.0.1:8000".parse()?;
/// let mut stream = TcpStream::connect(address)?;
///
/// let mut poller = Poller::new()?;
/// let mut events = Events::new();
///
/// // Register the socket with `Poller`.
/// poller.register(&mut stream, EventedId(0), TcpStream::INTERESTS, PollOption::Edge)?;
///
/// poller.poll(&mut events, None)?;
///
/// // The socket might be ready at this point.
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct TcpStream {
    inner: sys::TcpStream,
}

impl TcpStream {
    /// The interests to use when registering to receive both readable and
    /// writable events.
    pub const INTERESTS: Interests = Interests::BOTH;

    /// Create a new TCP stream and issue a non-blocking connect to the
    /// specified address.
    pub fn connect(addr: SocketAddr) -> io::Result<TcpStream> {
        sys::TcpStream::connect(addr).map(|inner| TcpStream { inner })
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    pub fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the socket address of the local half of this TCP connection.
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    pub fn set_ttl(&mut self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&mut self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    pub fn set_nodelay(&mut self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    pub fn nodelay(&mut self) -> io::Result<bool> {
        self.inner.nodelay()
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
    fn register(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        self.inner.register(poller, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        self.inner.reregister(poller, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut Poller) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

#[cfg(unix)]
impl FromRawFd for TcpStream {
    /// The caller must ensure that the stream is in non-blocking mode when
    /// using this function.
    unsafe fn from_raw_fd(fd: RawFd) -> TcpStream {
        TcpStream {
            inner: FromRawFd::from_raw_fd(fd),
        }
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

/// A TCP socket listener.
///
/// This works much like the `TcpListener` in the standard library, but this
/// doesn't block when calling [`accept`] and instead returns a [`WouldBlock`]
/// error.
///
/// [`accept`]: #method.accept
/// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Events, EventedId};
/// use mio_st::net::TcpListener;
/// use mio_st::poll::{Poller, PollOption};
///
/// let address = "127.0.0.1:8001".parse()?;
/// let mut listener = TcpListener::bind(address)?;
///
/// let mut poller = Poller::new()?;
/// let mut events = Events::new();
///
/// // Register the socket with `Poller`
/// poller.register(&mut listener, EventedId(0), TcpListener::INTERESTS, PollOption::Edge)?;
///
/// poller.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// // There may be a socket ready to be accepted.
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct TcpListener {
    inner: sys::TcpListener,
}

impl TcpListener {
    /// The interests to use when registering to receive acceptable connections
    /// events.
    pub const INTERESTS: Interests = Interests::READABLE;

    /// Convenience method to bind a new TCP listener to the specified address
    /// to receive new connections.
    pub fn bind(address: SocketAddr) -> io::Result<TcpListener> {
        sys::TcpListener::bind(address).map(|inner| TcpListener { inner })
    }

    /// Create a independently owned handle to the underlying socket.
    ///
    /// The returned `TcpListener` is a reference to the same socket as `self`.
    /// Both handles can be used to accept incoming connections and options set
    /// on one listener will affect the other.
    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.inner.try_clone().map(|inner| TcpListener { inner })
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
        self.inner.accept().map(|(inner, addr)| (TcpStream{ inner }, addr))
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    pub fn set_ttl(&mut self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&mut self) -> io::Result<u32> {
        self.inner.ttl()
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
    fn register(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "TcpListener only needs readable interests");
        self.inner.register(poller, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut Poller, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "TcpListener only needs readable interests");
        self.inner.reregister(poller, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut Poller) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

#[cfg(unix)]
impl FromRawFd for TcpListener {
    /// The caller must ensure that the listener is in non-blocking mode when
    /// using this function.
    unsafe fn from_raw_fd(fd: RawFd) -> TcpListener {
        TcpListener {
            inner: sys::TcpListener::from_raw_fd(fd),
        }
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
