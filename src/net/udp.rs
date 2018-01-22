use std::io;
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr};
#[cfg(unix)]
use std::os::unix::io::{IntoRawFd, AsRawFd, FromRawFd, RawFd};

use sys;
use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready};

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
/// use mio::event::{Events, EventedId};
/// use mio::net::UdpSocket;
/// use mio::poll::{Poll, PollOpt, Ready};
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
/// let mut events = Events::with_capacity(128);
///
/// // Register our sockets
/// poll.register(&mut sender_socket, SENDER, Ready::WRITABLE, PollOpt::EDGE)?;
/// poll.register(&mut echoer_socket, ECHOER, Ready::READABLE, PollOpt::EDGE)?;
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
    /// use mio::net::UdpSocket;
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
        let socket = net::UdpSocket::bind(addr)?;
        UdpSocket::from_socket(socket)
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
    pub fn from_socket(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        Ok(UdpSocket {
            socket: sys::UdpSocket::new(socket)?,
        })
    }

    /// Returns the socket address that this socket was created from.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
    ///
    /// assert_eq!(socket.local_addr()?, addr);
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
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
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
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
    pub fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
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
    /// use mio::net::UdpSocket;
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
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }

    /// Connects the UDP socket setting the default destination for `send()`
    /// and limiting packets that are read via `recv` from the address specified
    /// in `addr`.
    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.socket.connect(addr)
    }

    /// Sends data on the socket to the address previously bound via
    /// `connect()`. On success, returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    /// Receives data from the socket previously bound with `connect()`. On
    /// success, returns the number of bytes read and the address from whence
    /// the data came.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
    ///
    /// if socket.broadcast()? == false {
    ///     socket.set_broadcast(true)?;
    /// }
    ///
    /// assert_eq!(socket.broadcast()?, true);
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.socket.set_broadcast(on)
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see [`set_broadcast`].
    ///
    /// [`set_broadcast`]: #method.set_broadcast
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:7777".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
    ///
    /// assert_eq!(socket.broadcast()?, false);
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn broadcast(&self) -> io::Result<bool> {
        self.socket.broadcast()
    }

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v4(on)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v4`].
    ///
    /// [`set_multicast_loop_v4`]: #method.set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v4()
    }

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.socket.set_multicast_ttl_v4(ttl)
    }

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_ttl_v4`].
    ///
    /// [`set_multicast_ttl_v4`]: #method.set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.socket.multicast_ttl_v4()
    }

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v6(on)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_loop_v6`].
    ///
    /// [`set_multicast_loop_v6`]: #method.set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v6()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.socket.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: #method.set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        self.socket.ttl()
    }

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.socket.join_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    pub fn join_multicast_v6(&self, multiaddr: Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.join_multicast_v6(&multiaddr, interface)
    }

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see [`join_multicast_v4`].
    ///
    /// [`join_multicast_v4`]: #method.join_multicast_v4
    pub fn leave_multicast_v4(&self, multiaddr: Ipv4Addr, interface: Ipv4Addr) -> io::Result<()> {
        self.socket.leave_multicast_v4(&multiaddr, &interface)
    }

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see [`join_multicast_v6`].
    ///
    /// [`join_multicast_v6`]: #method.join_multicast_v6
    pub fn leave_multicast_v6(&self, multiaddr: Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.leave_multicast_v6(&multiaddr, interface)
    }

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// If this is set to `true` then the socket is restricted to sending and
    /// receiving IPv6 packets only. In this case two IPv4 and IPv6 applications
    /// can bind the same port at the same time.
    ///
    /// If this is set to `false` then the socket can be used to send and
    /// receive packets from an IPv4-mapped IPv6 address.
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.socket.set_only_v6(only_v6)
    }

    /// Gets the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see [`set_only_v6`].
    ///
    /// [`set_only_v6`]: #method.set_only_v6
    pub fn only_v6(&self) -> io::Result<bool> {
        self.socket.only_v6()
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.socket.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opts: PollOpt) -> io::Result<()> {
        self.socket.register(poll, id, interests, opts)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opts: PollOpt) -> io::Result<()> {
        self.socket.reregister(poll, id, interests, opts)
    }

    fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
        self.socket.deregister(poll)
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
