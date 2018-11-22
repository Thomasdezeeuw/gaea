use std::io;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use sys;
use event::{Evented, EventedId, Ready};
use poll::{PollCalled, PollOption, Poller};

/// A User Datagram Protocol socket.
///
/// This is an implementation of a bound UDP socket. This supports both IPv4 and
/// IPv6 addresses, and there is no corresponding notion of a server because UDP
/// is a datagram protocol.
///
/// If fine-grained control over the binding and options for a socket is desired
/// then use the `net2::UdpBuilder` methods, in the [`net2`] crate. This can be
/// used in combination with the [`UdpSocket::from_std_socket`] method to
/// transfer ownership into mio.
///
/// [`net2`]: https://crates.io/crates/net2
/// [`UdpSocket::from_std_socket`]: #method.from_std_socket
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
/// use mio_st::event::{Events, EventedId, Ready};
/// use mio_st::net::UdpSocket;
/// use mio_st::poll::{Poller, PollOption};
///
/// // Unique ids and address for both the sender and echoer.
/// const SENDER_ID: EventedId = EventedId(0);
/// const ECHOER_ID: EventedId = EventedId(1);
///
/// let sender_address = "127.0.0.1:7000".parse()?;
/// let echoer_address = "127.0.0.1:7001".parse()?;
///
/// // Create our sockets.
/// let mut sender_socket = UdpSocket::bind(sender_address)?;
/// let mut echoer_socket = UdpSocket::bind(echoer_address)?;
///
/// // Connect the sockets so we can use `send` and `recv`, rather then
/// // `send_to` and `recv_from`.
/// let mut sender_socket = sender_socket.connect(echoer_address)?;
/// let mut echoer_socket = echoer_socket.connect(sender_address)?;
///
/// // As always create our poll and events.
/// let mut poll = Poller::new()?;
/// let mut events = Events::new();
///
/// // Register our sockets
/// poll.register(&mut sender_socket, SENDER_ID, Ready::WRITABLE, PollOption::Level)?;
/// poll.register(&mut echoer_socket, ECHOER_ID, Ready::READABLE, PollOption::Level)?;
///
/// // The message we'll send.
/// const MSG_TO_SEND: &[u8; 11] = b"Hello world";
/// // A buffer for our echoer to receive the message in.
/// let mut buf = [0; 11];
///
/// // Our event loop.
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in &mut events {
///         match event.id() {
///             // Our sender is ready to send.
///             SENDER_ID => {
///                 let bytes_sent = sender_socket.send(MSG_TO_SEND)?;
///                 assert_eq!(bytes_sent, MSG_TO_SEND.len());
///                 println!("sent {:?} ({} bytes)", MSG_TO_SEND, bytes_sent);
///             },
///             // Our echoer is ready to read.
///             ECHOER_ID => {
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
    /// Creates a UDP socket and binds it to the given address.
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
    /// let address = "127.0.0.1:7002".parse()?;
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
    pub fn bind(address: SocketAddr) -> io::Result<UdpSocket> {
        sys::UdpSocket::bind(address).map(|socket| UdpSocket { socket })
    }

    /// Connects the UDP socket by setting the default destination and limiting
    /// packets that are read, written and peeked to the address specified in
    /// `addr`.
    ///
    /// See [`ConnectedUdpSocket`] for more information.
    ///
    /// [`ConnectedUdpSocket`]: struct.ConnectedUdpSocket.html
    pub fn connect(self, addr: SocketAddr) -> io::Result<ConnectedUdpSocket> {
        self.socket.connect(addr)
            .map(|_| ConnectedUdpSocket { socket: self.socket })
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
    /// let addr = "127.0.0.1:7003".parse()?;
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

    /// Sends data to the given address. On success, returns the number of bytes
    /// written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7004".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// // We must check if the socket is writable before calling send_to,
    /// // or we could run into a WouldBlock error.
    ///
    /// let other_addr = "127.0.0.1:7005".parse()?;
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
    /// let addr = "127.0.0.1:7006".parse()?;
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
    /// let addr = "127.0.0.1:7007".parse()?;
    /// let mut socket = UdpSocket::bind(addr)?;
    ///
    /// // We must check if the socket is readable before calling recv_from,
    /// // or we could run into a WouldBlock error.
    ///
    /// let mut buf1 = [0; 9];
    /// let mut buf2 = [0; 9];
    /// let (num_recv1, from_addr1) = socket.peek_from(&mut buf1)?;
    /// let (num_recv2, from_addr2) = socket.recv_from(&mut buf2)?;
    /// assert_eq!(num_recv1, num_recv2);
    /// assert_eq!(from_addr1, from_addr2);
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
    fn register(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        self.socket.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        self.socket.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poller, p: PollCalled) -> io::Result<()> {
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

/// A connected User Datagram Protocol socket.
///
/// This connected variant of a `UdpSocket` and can be created by calling
/// [`connect`] on a [`UdpSocket`].
///
/// Also see [`UdpSocket`] for more creating a socket, including setting various
/// options.
///
/// [`connect`]: struct.UdpSocket.html#method.connect
/// [`UdpSocket`]: struct.UdpSocket.html
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio_st::event::{Events, EventedId, Ready};
/// use mio_st::net::{ConnectedUdpSocket, UdpSocket};
/// use mio_st::poll::{Poller, PollOption};
///
/// const ECHOER_ID: EventedId = EventedId(0);
/// const SENDER_ID: EventedId = EventedId(1);
///
/// // Create our echoer.
/// let echoer_addr = "127.0.0.1:7008".parse()?;
/// let mut echoer = UdpSocket::bind(echoer_addr)?;
///
/// // Then we connect to the server.
/// let sender_addr = "127.0.0.1:7009".parse()?;
/// let mut sender = ConnectedUdpSocket::connect(sender_addr, echoer_addr)?;
///
/// // Create our poll instance and events container.
/// let mut poll = Poller::new()?;
/// let mut events = Events::new();
///
/// // Register our echoer and sender.
/// poll.register(&mut echoer, ECHOER_ID, Ready::READABLE, PollOption::Level)?;
/// poll.register(&mut sender, SENDER_ID, Ready::WRITABLE, PollOption::Level)?;
///
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in &mut events {
///         match event.id() {
///             ECHOER_ID => {
///                 let mut buf = [0; 20];
///                 let (recv_n, addr) = echoer.recv_from(&mut buf)?;
///                 println!("Received: {:?} from {}", &buf[0..recv_n], addr);
/// #               return Ok(());
///             },
///             SENDER_ID => {
///                 let msg = b"hello world";
///                 sender.send(msg)?;
///             },
///             _ => unreachable!(),
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
pub struct ConnectedUdpSocket {
    socket: sys::UdpSocket,
}

impl ConnectedUdpSocket {
    /// Creates a connected UDP socket.
    ///
    /// This method first binds a UDP socket to the `bind_addr`ess, then connects
    /// that socket to `connect_addr`ess. The is convenience method for a call
    /// to `UdpSocket::bind` followed by a call to `connect`.
    pub fn connect(bind_addr: SocketAddr, connect_addr: SocketAddr) -> io::Result<ConnectedUdpSocket> {
        UdpSocket::bind(bind_addr)
            .and_then(|socket| socket.connect(connect_addr))
    }

    /// Creates a new mio-wrapped UDP socket from an bound and connected UDP
    /// socket from the standard library.
    ///
    /// This function requires that `socket` has previously been bound and
    /// connected to an address to work correctly.
    pub fn from_connected_std_socket(socket: net::UdpSocket) -> io::Result<ConnectedUdpSocket> {
        socket.set_nonblocking(true)?;
        sys::UdpSocket::new(socket)
            .map(|socket| ConnectedUdpSocket { socket })
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
    /// let addr = "127.0.0.1:7010".parse()?;
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

    /// Sends data on the socket to the connected socket. On success, returns
    /// the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::ConnectedUdpSocket;
    ///
    /// let local_addr = "127.0.0.1:7011".parse()?;
    /// let remote_addr = "127.0.0.1:7012".parse()?;
    /// let mut socket = ConnectedUdpSocket::connect(local_addr, remote_addr)?;
    ///
    /// // We must check if the socket is writable before calling send, or we
    /// // could run into a WouldBlock error.
    ///
    /// let bytes_sent = socket.send(&[9; 9])?;
    /// assert_eq!(bytes_sent, 9);
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::ConnectedUdpSocket;
    ///
    /// let local_addr = "127.0.0.1:7013".parse()?;
    /// let remote_addr = "127.0.0.1:7014".parse()?;
    /// let mut socket = ConnectedUdpSocket::connect(local_addr, remote_addr)?;
    ///
    /// // We must check if the socket is readable before calling recv, or we
    /// // could run into a WouldBlock error.
    ///
    /// let mut buf = [0; 9];
    /// let num_recv = socket.recv(&mut buf)?;
    /// println!("Received {:?} -> {:?} bytes", buf, num_recv);
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio_st::net::ConnectedUdpSocket;
    ///
    /// let local_addr = "127.0.0.1:7015".parse()?;
    /// let remote_addr = "127.0.0.1:7016".parse()?;
    /// let mut socket = ConnectedUdpSocket::connect(local_addr, remote_addr)?;
    ///
    /// // We must check if the socket is readable before calling peek, or we
    /// // could run into a WouldBlock error.
    ///
    /// let mut buf1 = [0; 9];
    /// let mut buf2 = [0; 9];
    /// let num_recv1 = socket.peek(&mut buf1)?;
    /// let num_recv2 = socket.recv(&mut buf2)?;
    /// assert_eq!(buf1, buf2);
    /// assert_eq!(num_recv1, num_recv2);
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
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

impl Evented for ConnectedUdpSocket {
    fn register(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        self.socket.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        self.socket.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poller, p: PollCalled) -> io::Result<()> {
        self.socket.deregister(poll, p)
    }
}

#[cfg(unix)]
impl IntoRawFd for ConnectedUdpSocket {
    fn into_raw_fd(self) -> RawFd {
        self.socket.into_raw_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for ConnectedUdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

#[cfg(unix)]
impl FromRawFd for ConnectedUdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> ConnectedUdpSocket {
        ConnectedUdpSocket {
            socket: FromRawFd::from_raw_fd(fd),
        }
    }
}
