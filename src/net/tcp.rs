use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::os::{Evented, Interests, OsQueue, RegisterOption};
use crate::{event, sys};

/// A non-blocking TCP stream between a local socket and a remote socket.
///
/// This works much like the `TcpStream` in the standard library, but the
/// [`Read`] and [`Write`] implementation don't block and instead return a
/// [`WouldBlock`] error.
///
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
///
/// # Deregistering
///
/// `TcpStream` will deregister itself when dropped.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::io;
///
/// use gaea::{event, poll};
/// use gaea::net::TcpStream;
/// # use gaea::net::TcpListener;
/// use gaea::os::{OsQueue, RegisterOption};
///
/// let address = "127.0.0.1:8999".parse()?;
/// # let listener = TcpListener::bind(address)?;
/// let mut stream = TcpStream::connect(address)?;
///
/// let mut os_queue = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// // Register the socket with `OsQueue`.
/// os_queue.register(&mut stream, event::Id(0), TcpStream::INTERESTS, RegisterOption::EDGE)?;
///
/// poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
///
/// // If event ID 0 was returned by `poll` then the stream will be ready to
/// // read or write.
/// # drop(listener);
/// # Ok(())
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
    pub fn connect(address: SocketAddr) -> io::Result<TcpStream> {
        sys::TcpStream::connect(address).map(|inner| TcpStream { inner })
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
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.inner.register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.inner.reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        self.inner.deregister(os_queue)
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
/// [`accept`]: TcpListener::accept
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
///
/// # Deregistering
///
/// `TcpListener` will deregister itself when dropped, **iff** it is not cloned
/// (via [`try_clone`]).
///
/// [`try_clone`]: TcpListener::try_clone
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::io;
/// use std::time::Duration;
///
/// use gaea::{event, poll};
/// use gaea::net::TcpListener;
/// use gaea::os::{OsQueue, RegisterOption};
///
/// let address = "127.0.0.1:8001".parse()?;
/// let mut listener = TcpListener::bind(address)?;
///
/// let mut os_queue = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// const LISTENER_ID: event::Id = event::Id(0);
///
/// // Register the socket with `OsQueue`.
/// os_queue.register(&mut listener, LISTENER_ID, TcpListener::INTERESTS, RegisterOption::EDGE)?;
///
/// // Poll for new events.
/// poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(100)))?;
///
/// // If `LISTENER_ID` was returned by `poll` then the listener will be ready
/// // to accept connection.
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
    ///
    /// This also sets the `SO_REUSEPORT` and `SO_REUSEADDR` options on the
    /// socket.
    pub fn bind(address: SocketAddr) -> io::Result<TcpListener> {
        sys::TcpListener::bind(address).map(|inner| TcpListener { inner })
    }

    /// Create a independently owned handle to the underlying socket.
    ///
    /// The returned `TcpListener` is a reference to the same socket as `self`.
    /// Both handles can be used to accept incoming connections and options set
    /// on one listener will affect the other.
    ///
    /// # Notes
    ///
    /// On Linux when a `TcpListener` is cloned it must deregistered. If its not
    /// deregistered explicitly and one listener is closed (dropped) and another
    /// is still open the os queue will still receive events.
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
    /// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
    pub fn accept(&mut self) -> io::Result<(TcpStream, SocketAddr)> {
        self.inner.accept().map(|(inner, address)| (TcpStream{ inner }, address))
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
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "TcpListener only needs readable interests");
        self.inner.register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "TcpListener only needs readable interests");
        self.inner.reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        self.inner.deregister(os_queue)
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
