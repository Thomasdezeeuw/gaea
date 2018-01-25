use std::io;
use std::net::{self, SocketAddr};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};

use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready, Private};
use sys::unix::EventedFd;

#[derive(Debug)]
pub struct UdpSocket {
    socket: net::UdpSocket,
}

impl UdpSocket {
    pub fn new(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        Ok(UdpSocket { socket })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn send_to(&self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(buf, target)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.peek_from(buf)
    }

    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.socket.connect(addr)
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.peek(buf)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.socket.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opt: PollOpt, p: Private) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poll, p: Private) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll, p)
    }
}

impl FromRawFd for UdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> UdpSocket {
        UdpSocket {
            socket: net::UdpSocket::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for UdpSocket {
    fn into_raw_fd(self) -> RawFd {
        self.socket.into_raw_fd()
    }
}

impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}
