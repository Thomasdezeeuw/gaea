use std::io::{self, Read, Write};
use std::mem::size_of_val;
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::event;
use crate::os::{Evented, Interests, PollOption, OsQueue};
use crate::sys::unix::eventedfd::EventedFd;

#[derive(Debug)]
pub struct TcpStream {
    stream: net::TcpStream,
}

impl TcpStream {
    pub fn connect(address: SocketAddr) -> io::Result<TcpStream> {
        // Create a raw socket file descriptor.
        let socket_family = match address {
            SocketAddr::V4(..) => libc::AF_INET,
            SocketAddr::V6(..) => libc::AF_INET6,
        };
        let socket_fd = unsafe { libc::socket(socket_family, libc::SOCK_STREAM, 0) };
        if socket_fd == -1 {
            return Err(io::Error::last_os_error());
        }

        // Set non blocking mode.
        if unsafe { libc::fcntl(socket_fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
            return Err(io::Error::last_os_error());
        }

        // Connect to the provided address. If this would block it will return
        // `EINPROGRESS`, which we don't consider an error here.
        let (raw_address, raw_address_length) = raw_address(&address);
        if unsafe { libc::connect(socket_fd, raw_address, raw_address_length) } == -1 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(err);
            }
        }

        let stream = unsafe { net::TcpStream::from_raw_fd(socket_fd) };
        Ok(TcpStream { stream })
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.stream.local_addr()
    }

    pub fn set_ttl(&mut self, ttl: u32) -> io::Result<()> {
        self.stream.set_ttl(ttl)
    }

    pub fn ttl(&mut self) -> io::Result<u32> {
        self.stream.ttl()
    }

    pub fn set_nodelay(&mut self, nodelay: bool) -> io::Result<()> {
        self.stream.set_nodelay(nodelay)
    }

    pub fn nodelay(&mut self) -> io::Result<bool> {
        self.stream.nodelay()
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.peek(buf)
    }

    pub fn shutdown(&self, how: net::Shutdown) -> io::Result<()> {
        self.stream.shutdown(how)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.stream.take_error()
    }
}

// Implementation taken from the Rust standard library.
// Copyright 2015 The Rust Project Developers.
#[allow(trivial_casts)]
fn raw_address(address: &SocketAddr) -> (*const libc::sockaddr, libc::socklen_t) {
    match *address {
        SocketAddr::V4(ref address) => {
            (address as *const _ as *const _, size_of_val(address) as libc::socklen_t)
        }
        SocketAddr::V6(ref address) => {
            (address as *const _ as *const _, size_of_val(address) as libc::socklen_t)
        }
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl Evented for TcpStream {
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(os_queue)
    }
}

impl Into<net::TcpStream> for TcpStream {
    fn into(self) -> net::TcpStream {
        self.stream
    }
}

impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpStream {
        TcpStream {
            stream: net::TcpStream::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for TcpStream {
    fn into_raw_fd(self) -> RawFd {
        self.stream.into_raw_fd()
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}

#[derive(Debug)]
pub struct TcpListener {
    listener: net::TcpListener,
}

impl TcpListener {
    pub fn bind(address: SocketAddr) -> io::Result<TcpListener> {
        // Create a raw socket file descriptor.
        let socket_family = match address {
            SocketAddr::V4(..) => libc::AF_INET,
            SocketAddr::V6(..) => libc::AF_INET6,
        };
        let socket_fd = unsafe { libc::socket(socket_family, libc::SOCK_STREAM, 0) };
        if socket_fd == -1 {
            return Err(io::Error::last_os_error());
        }

        // Set the `SO_REUSEPORT` and `SO_REUSEADDR` options.
        unsafe {
            enable_socket_option(socket_fd, libc::SOL_SOCKET, libc::SO_REUSEPORT)?;
            enable_socket_option(socket_fd, libc::SOL_SOCKET, libc::SO_REUSEADDR)?;
        }

        // Set non blocking mode.
        if unsafe { libc::fcntl(socket_fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
            return Err(io::Error::last_os_error());
        }

        // Bind to the address
        let (raw_address, raw_address_length) = raw_address(&address);
        if unsafe { libc::bind(socket_fd, raw_address, raw_address_length) } == -1 {
            return Err(io::Error::last_os_error());
        }

        // Mark the socket as passive.
        if unsafe { libc::listen(socket_fd, 128) } == -1 {
            return Err(io::Error::last_os_error());
        }

        let listener = unsafe { net::TcpListener::from_raw_fd(socket_fd) };
        Ok(TcpListener { listener })
    }

    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.listener.try_clone().map(|listener| TcpListener { listener })
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let (stream, address) = self.listener.accept()?;
        stream.set_nonblocking(true)?;
        Ok((TcpStream { stream }, address))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn set_ttl(&mut self, ttl: u32) -> io::Result<()> {
        self.listener.set_ttl(ttl)
    }

    pub fn ttl(&mut self) -> io::Result<u32> {
        self.listener.ttl()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.listener.take_error()
    }
}

/// Enable a socket option via `setsockopt`.
#[allow(trivial_casts)]
unsafe fn enable_socket_option(fd: RawFd, level: libc::c_int, name: libc::c_int) -> io::Result<()> {
    let enable: libc::c_int = 1;
    let err = libc::setsockopt(fd, level, name,
        (&enable as *const i32) as *const libc::c_void,
        size_of_val(&enable) as libc::socklen_t);
    if err == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl Evented for TcpListener {
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(os_queue)
    }
}

impl FromRawFd for TcpListener {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpListener {
        TcpListener {
            listener: net::TcpListener::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for TcpListener {
    fn into_raw_fd(self) -> RawFd {
        self.listener.into_raw_fd()
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }
}
