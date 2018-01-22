use std::io;
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};
use net2::UdpSocketExt;

use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready};
use sys::unix::EventedFd;

#[derive(Debug)]
pub struct UdpSocket {
    socket: net::UdpSocket,
}

impl UdpSocket {
    pub fn new(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        socket.set_nonblocking(true)?;
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

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.recv(buf)
    }

    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.socket.connect(addr)
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.socket.broadcast()
    }

    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.socket.set_broadcast(on)
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v4()
    }

    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v4(on)
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.socket.multicast_ttl_v4()
    }

    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.socket.set_multicast_ttl_v4(ttl)
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v6()
    }

    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v6(on)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.socket.ttl()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.socket.set_ttl(ttl)
    }

    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.socket.join_multicast_v4(multiaddr, interface)
    }

    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.join_multicast_v6(multiaddr, interface)
    }

    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.socket.leave_multicast_v4(multiaddr, interface)
    }

    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.leave_multicast_v6(multiaddr, interface)
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.socket.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.socket.only_v6()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.socket.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, id, interests, opts)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, id, interests, opts)
    }

    fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
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
