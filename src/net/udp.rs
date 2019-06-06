use std::io;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::os::{Evented, Interests, OsQueue, RegisterOption};
use crate::{event, sys};

/// A User Datagram Protocol socket.
///
/// This works much like the `UdpSocket` in the standard library, but the I/O
/// methods such as [`send_to`], [`send`] etc. don't block and instead return a
/// [`WouldBlock`] error.
///
/// [`send_to`]: UdpSocket::send_to
/// [`send`]: UdpSocket::send
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
///
/// # Deregistering
///
/// `UdpSocket` will deregister itself when dropped.
///
/// # Examples
///
/// An simple echo program, the `sender` sends a message and the `echoer`
/// listens for messages and prints them to standard out.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::io;
///
/// use mio_st::{event, poll};
/// use mio_st::net::UdpSocket;
/// use mio_st::os::{Interests, RegisterOption, OsQueue};
///
/// // Unique ids and addresses for both the sender and echoer.
/// const SENDER_ID: event::Id = event::Id(0);
/// const ECHOER_ID: event::Id = event::Id(1);
///
/// let sender_address = "127.0.0.1:7000".parse()?;
/// let echoer_address = "127.0.0.1:7001".parse()?;
///
/// // Create our sockets.
/// let mut sender_socket = UdpSocket::bind(sender_address)?;
/// let mut echoer_socket = UdpSocket::bind(echoer_address)?;
///
/// // Connect the sending socket so we can use `send` method.
/// sender_socket.connect(echoer_address)?;
///
/// // As always create our poll and events.
/// let mut os_queue = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// // Register our sockets
/// os_queue.register(&mut sender_socket, SENDER_ID, Interests::WRITABLE, RegisterOption::LEVEL)?;
/// os_queue.register(&mut echoer_socket, ECHOER_ID, Interests::READABLE, RegisterOption::LEVEL)?;
///
/// // The message we'll send.
/// const MSG_TO_SEND: &[u8; 11] = b"Hello world";
/// // A buffer for our echoer to receive the message in.
/// let mut buf = [0; 20];
///
/// // Our event loop.
/// loop {
///     // Poll for events.
///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
///
///     for event in &mut events {
///         match event.id() {
///             SENDER_ID => {
///                 // Our sender is ready to send.
///                 let bytes_sent = sender_socket.send(MSG_TO_SEND)?;
///                 assert_eq!(bytes_sent, MSG_TO_SEND.len());
///                 println!("sent {:?} ({} bytes)", MSG_TO_SEND, bytes_sent);
///             },
///             ECHOER_ID => {
///                 // Our echoer is ready to read.
///                 let (bytes_recv, address) = echoer_socket.recv_from(&mut buf)?;
///                 println!("received {:?} ({} bytes) from {}", &buf[0..bytes_recv], bytes_recv, address);
///                 # return Ok(());
///             }
///             // We shouldn't receive any event with another id then the two
///             // defined above.
///             _ => unreachable!("received an unexpected event")
///         }
///     }
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct UdpSocket {
    socket: sys::UdpSocket,
}

impl UdpSocket {
    /// The interests to use when registering to receive both readable and
    /// writable events.
    pub const INTERESTS: Interests = Interests::BOTH;

    /// Creates a UDP socket and binds it to the given address.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// // We must bind it to an open address.
    /// let address = "127.0.0.1:7002".parse()?;
    /// let socket = UdpSocket::bind(address)?;
    ///
    /// // Our socket was created, but we should not use it before checking it's
    /// // readiness.
    /// #    drop(socket); // Silence unused variable warning.
    /// #    Ok(())
    /// # }
    /// ```
    pub fn bind(address: SocketAddr) -> io::Result<UdpSocket> {
        sys::UdpSocket::bind(address).map(|socket| UdpSocket { socket })
    }

    /// Connects the UDP socket by setting the default destination and limiting
    /// packets that are read, written and peeked to the address specified in
    /// `address`.
    ///
    /// This allows the [`send`], [`recv`] and [`peek`] methods to be used.
    ///
    /// [`send`]: UdpSocket::send
    /// [`recv`]: UdpSocket::recv
    /// [`peek`]: UdpSocket::peek
    pub fn connect(&mut self, address: SocketAddr) -> io::Result<()> {
        self.socket.connect(address)
    }

    /// Returns the socket address that this socket was created from.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mio_st::net::UdpSocket;
    ///
    /// let address = "127.0.0.1:7003".parse()?;
    /// let mut socket = UdpSocket::bind(address)?;
    ///
    /// assert_eq!(socket.local_addr()?, address);
    /// #    Ok(())
    /// # }
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Sends data to the given address. On success, returns the number of bytes
    /// written.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let address = "127.0.0.1:7004".parse()?;
    /// let mut socket = UdpSocket::bind(address)?;
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::WRITABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let other_address = "127.0.0.1:7005".parse()?;
    /// let bytes_sent = socket.send_to(b"Hello world", other_address)?;
    /// assert_eq!(bytes_sent, 11);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn send_to(&mut self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(buf, &target)
    }

    /// Sends data on the socket to the connected socket. On success, returns
    /// the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let local_address = "127.0.0.1:7006".parse()?;
    /// let remote_address = "127.0.0.1:7007".parse()?;
    /// let mut socket = UdpSocket::bind(local_address)?;
    /// socket.connect(remote_address)?;
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::WRITABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let bytes_sent = socket.send(b"Hello world")?;
    /// assert_eq!(bytes_sent, 11);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// This requires the socket to be [connected].
    ///
    /// [connected]: UdpSocket::connect
    pub fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let address = "127.0.0.1:7008".parse()?;
    /// let mut socket = UdpSocket::bind(address)?;
    /// #
    /// # // Send some data that we can receive.
    /// # let mut socket2 = UdpSocket::bind("127.0.0.1:7108".parse()?)?;
    /// # os_queue.register(&mut socket2, event::Id(1), Interests::WRITABLE, RegisterOption::EDGE)?;
    /// # while events.is_empty() { poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?; }
    /// # let bytes_sent = socket2.send_to(b"Hello world", address)?;
    /// # assert_eq!(bytes_sent, 11);
    /// # events.clear();
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::READABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let mut buf = [0; 20];
    /// let (bytes_received, from_address) = socket.recv_from(&mut buf)?;
    /// println!("Received {:?} ({} bytes) from {}", &buf[..bytes_received], bytes_received, from_address);
    /// # assert_eq!(&buf[..bytes_received], b"Hello world");
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let local_address = "127.0.0.1:7009".parse()?;
    /// let remote_address = "127.0.0.1:7010".parse()?;
    /// let mut socket = UdpSocket::bind(local_address)?;
    /// #
    /// # // Send some data that we can receive.
    /// # let mut socket2 = UdpSocket::bind(remote_address)?;
    /// # os_queue.register(&mut socket2, event::Id(1), Interests::WRITABLE, RegisterOption::EDGE)?;
    /// # while events.is_empty() { poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?; }
    /// # let bytes_sent = socket2.send_to(b"Hello world", local_address)?;
    /// # assert_eq!(bytes_sent, 11);
    /// # events.clear();
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::READABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let mut buf = [0; 20];
    /// let bytes_received = socket.recv(&mut buf)?;
    /// println!("Received {:?} ({} bytes)", &buf[..bytes_received], bytes_received);
    /// # assert_eq!(&buf[..bytes_received], b"Hello world");
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// This requires the socket to be [connected].
    ///
    /// [connected]: UdpSocket::connect
    pub fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read and the address from whence
    /// the data came.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let address = "127.0.0.1:7011".parse()?;
    /// let mut socket = UdpSocket::bind(address)?;
    /// #
    /// # // Send some data that we can receive.
    /// # let mut socket2 = UdpSocket::bind("127.0.0.1:7111".parse()?)?;
    /// # os_queue.register(&mut socket2, event::Id(1), Interests::WRITABLE, RegisterOption::EDGE)?;
    /// # while events.is_empty() { poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?; }
    /// # let bytes_sent = socket2.send_to(b"Hello world", address)?;
    /// # assert_eq!(bytes_sent, 11);
    /// # events.clear();
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::READABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let mut buf1 = [0; 20];
    /// let (bytes_received1, from_address1) = socket.peek_from(&mut buf1)?;
    /// println!("Peeked {:?} ({} bytes) from {}", &buf1[..bytes_received1], bytes_received1, from_address1);
    /// # assert_eq!(&buf1[..bytes_received1], b"Hello world");
    /// # assert_eq!(from_address1, "127.0.0.1:7111".parse()?);
    ///
    /// let mut buf2 = [0; 20];
    /// let (bytes_received2, from_address2) = socket.recv_from(&mut buf2)?;
    /// println!("Received {:?} ({} bytes) from {}", &buf2[..bytes_received2], bytes_received2, from_address2);
    /// assert_eq!(bytes_received1, bytes_received2);
    /// assert_eq!(&buf1[..bytes_received1], &buf2[..bytes_received2]);
    /// assert_eq!(from_address1, from_address2);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn peek_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.peek_from(buf)
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::io;
    ///
    /// use mio_st::net::UdpSocket;
    /// use mio_st::os::{RegisterOption, Interests};
    /// use mio_st::{event, OsQueue, poll};
    ///
    /// let mut os_queue = OsQueue::new()?;
    /// let mut events = Vec::new();
    ///
    /// let local_address = "127.0.0.1:7012".parse()?;
    /// let remote_address = "127.0.0.1:7013".parse()?;
    /// let mut socket = UdpSocket::bind(local_address)?;
    /// socket.connect(remote_address)?;
    /// #
    /// # // Send some data that we can receive.
    /// # let mut socket2 = UdpSocket::bind(remote_address)?;
    /// # os_queue.register(&mut socket2, event::Id(1), Interests::WRITABLE, RegisterOption::EDGE)?;
    /// # while events.is_empty() { poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?; }
    /// # let bytes_sent = socket2.send_to(b"Hello world", local_address)?;
    /// # assert_eq!(bytes_sent, 11);
    /// # events.clear();
    ///
    /// // Register our socket.
    /// os_queue.register(&mut socket, event::Id(0), Interests::READABLE, RegisterOption::EDGE)?;
    ///
    /// // Poll until our socket is ready.
    /// while events.is_empty() {
    ///     poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
    /// }
    ///
    /// let mut buf1 = [0; 20];
    /// let bytes_received1 = socket.peek(&mut buf1)?;
    /// println!("Peeked {:?} ({} bytes)", &buf1[..bytes_received1], bytes_received1);
    /// # assert_eq!(&buf1[..bytes_received1], b"Hello world");
    ///
    /// let mut buf2 = [0; 20];
    /// let bytes_received2 = socket.recv(&mut buf2)?;
    /// println!("Received {:?} ({} bytes)", &buf2[..bytes_received2], bytes_received2);
    /// assert_eq!(bytes_received1, bytes_received2);
    /// assert_eq!(&buf1[..bytes_received1], &buf2[..bytes_received2]);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// This requires the socket to be [connected].
    ///
    /// [connected]: UdpSocket::connect
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
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.socket.register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.socket.reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        self.socket.deregister(os_queue)
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
