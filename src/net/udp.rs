use std::io;
use std::net::{self, SocketAddr};
#[cfg(unix)]
use std::os::unix::io::{IntoRawFd, AsRawFd, FromRawFd, RawFd};

use sys;
use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready, Private};

/// A User Datagram Protocol socket.
///
/// This is an implementation of a bound UDP socket. This supports both IPv4 and
/// IPv6 addresses, and there is no corresponding notion of a server because UDP
/// is a datagram protocol.
///
/// # Examples
///
/// An simple echo program, the `sender` sends a message and the `echoer`
/// listens for messages and prints them to standard out.
///
/// ```
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Events, EventedId};
/// use mio_st::net::UdpSocket;
/// use mio_st::poll::{Poll, PollOpt, Ready};
///
/// // Unique ids and address for both the sender and echoer.
/// const SENDER: EventedId = EventedId(0);
/// const ECHOER: EventedId = EventedId(1);
///
/// let sender_address = "127.0.0.1:7777".parse()?;
/// let echoer_address = "127.0.0.1:8888".parse()?;
///
/// // Create our sockets.
/// let mut sender_socket = UdpSocket::bind(sender_address)?;
/// let mut echoer_socket = UdpSocket::bind(echoer_address)?;
///
/// // Connect the sender so we can use `send` and `recv`, rather then `send_to`
/// // and `recv_from`.
/// sender_socket.connect(echoer_address)?;
///
/// // As always create our poll and events.
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(16, 16);
///
/// // Register our sockets
/// poll.register(&mut sender_socket, SENDER, Ready::WRITABLE, PollOpt::Edge)?;
/// poll.register(&mut echoer_socket, ECHOER, Ready::READABLE, PollOpt::Edge)?;
///
/// // The message we'll send.
/// const MSG_TO_SEND: &[u8; 11] = b"Hello world";
/// // A buffer for our echoer to receive the message in.
/// let mut buf = [0; 11];
///
/// // Our event loop.
/// loop {
///     poll.poll(&mut events, None)?;
///     for event in &mut events {
///         match event.id() {
///             // Our sender is ready to send.
///             SENDER => {
///                 let bytes_sent = sender_socket.send(MSG_TO_SEND)?;
///                 assert_eq!(bytes_sent, MSG_TO_SEND.len());
///                 println!("sent {:?} ({} bytes)", MSG_TO_SEND, bytes_sent);
///             },
///             // Our echoer is ready to read.
///             ECHOER => {
///                 let bytes_recv = echoer_socket.recv(&mut buf)?;
///                 println!("{:?} ({} bytes)", &buf[0..bytes_recv], bytes_recv);
///                 # return Ok(());
///             }
///             // We shouldn't receive any event with another id then the two
///             // defined above.
///             _ => unreachable!("received an unexpected event")
///         }
///     }
/// }
/// # }
/// #
/// # fn main() {
/// #   try_main().unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct UdpSocket {
    socket: sys::UdpSocket,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// // We must bind it to an open address.
    /// let address = "127.0.0.1:7777".parse()?;
    /// let socket = UdpSocket::bind(address)?;
    ///
    /// // Our socket was created, but we should not use it before checking it's
    /// // readiness.
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn bind(addr: SocketAddr) -> io::Result<UdpSocket> {
        net::UdpSocket::bind(addr)
            .and_then(UdpSocket::from_std_socket)
    }

    /// Creates a new mio-wrapped socket from an underlying and bound std
    /// socket.
    ///
    /// This function requires that `socket` has previously been bound to an
    /// address to work correctly, and returns an I/O object which can be used
    /// with mio to send/receive UDP messages.
    ///
    /// This can be used in conjunction with net2's `UdpBuilder` interface to
    /// configure a socket before it's handed off to mio, such as setting
    /// options like `reuse_address` or binding to multiple addresses.
    pub fn from_std_socket(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        socket.set_nonblocking(true)?;
        sys::UdpSocket::new(socket)
            .map(|socket| UdpSocket { socket, })
    }

    /// Returns the socket address that this socket was created from.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// assert_eq!(socket.local_addr()?, addr);
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// // We must check if the socket is writable before calling send_to,
    /// // or we could run into a WouldBlock error.
    ///
    /// let other_addr = "127.0.0.1:8888".parse()?;
    /// let bytes_sent = socket.send_to(&[9; 9], other_addr)?;
    /// assert_eq!(bytes_sent, 9);
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn send_to(&mut self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(buf, &target)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// // We must check if the socket is readable before calling recv_from,
    /// // or we could run into a WouldBlock error.
    ///
    /// let mut buf = [0; 9];
    /// let (num_recv, from_addr) = socket.recv_from(&mut buf)?;
    /// println!("Received {:?} -> {:?} bytes from {:?}", buf, num_recv, from_addr);
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read and the address from whence
    /// the data came.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// // We must check if the socket is readable before calling recv_from,
    /// // or we could run into a WouldBlock error.
    ///
    /// let mut buf1 = [0; 9];
    /// let mut buf2 = [0; 9];
    /// let (num_recv, from_addr) = socket.peek_from(&mut buf1)?;
    /// let (num_recv, from_addr) = socket.recv_from(&mut buf2)?;
    /// assert_eq!(buf1, buf2);
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn peek_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.peek_from(buf)
    }

    /// Connects the UDP socket setting the default destination for `send()` and
    /// limiting packets that are read, writen and peeked to the address
    /// specified in `addr`.
    pub fn connect(&mut self, addr: SocketAddr) -> io::Result<()> {
        self.socket.connect(addr)
    }

    /// Sends data on the socket to the address previously bound via
    /// `connect()`. On success, returns the number of bytes written.
    pub fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    /// Receives data from the socket previously bound with `connect()`. On
    /// success, returns the number of bytes read.
    pub fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    /// Receives data from the socket previously bound with `connect()`, without
    /// removing it from the input queue. On success, returns the number of
    /// bytes read.
    pub fn peek(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.peek(buf)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&mut self) -> io::Result<Option<io::Error>> {
        self.socket.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.socket.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        self.socket.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
        self.socket.deregister(poll, p)
    }
}

#[cfg(unix)]
impl IntoRawFd for UdpSocket {
    fn into_raw_fd(self) -> RawFd {
        self.socket.into_raw_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

#[cfg(unix)]
impl FromRawFd for UdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> UdpSocket {
        UdpSocket {
            socket: FromRawFd::from_raw_fd(fd),
        }
    }
}
