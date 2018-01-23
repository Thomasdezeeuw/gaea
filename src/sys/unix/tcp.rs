use std::io::{self, Read, Write};
use std::net::{self, SocketAddr};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};
use std::time::Duration;

use libc;
use net2::TcpStreamExt;

use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready, Private};
use sys::unix::eventedfd::EventedFd;

#[derive(Debug)]
pub struct TcpStream {
    stream: net::TcpStream,
}

impl TcpStream {
    pub fn connect(stream: net::TcpStream, addr: &SocketAddr) -> io::Result<TcpStream> {
        stream.set_nonblocking(true)?;

        match stream.connect(addr) {
            Ok(..) => {},
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {},
            Err(e) => return Err(e),
        }

        Ok(TcpStream { stream })
    }

    pub fn from_stream(stream: net::TcpStream) -> TcpStream {
        TcpStream { stream }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.stream.local_addr()
    }

    pub fn shutdown(&self, how: net::Shutdown) -> io::Result<()> {
        self.stream.shutdown(how)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.stream.set_nodelay(nodelay)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.stream.nodelay()
    }

    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.stream.set_recv_buffer_size(size)
    }

    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.stream.recv_buffer_size()
    }

    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.stream.set_send_buffer_size(size)
    }

    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.stream.send_buffer_size()
    }

    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.stream.set_keepalive(keepalive)
    }

    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.stream.keepalive()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.stream.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.stream.ttl()
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.stream.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.stream.only_v6()
    }

    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.stream.set_linger(dur)
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.stream.linger()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.stream.take_error()
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.peek(buf)
    }
}

impl<'a> Read for &'a TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.stream).read(buf)
    }
}

impl<'a> Write for &'a TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.stream).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.stream).flush()
    }
}

impl Evented for TcpStream {
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
    pub fn new(listener: net::TcpListener, _addr: &SocketAddr) -> io::Result<TcpListener> {
        listener.set_nonblocking(true)?;
        Ok(TcpListener { listener })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn accept(&self) -> io::Result<(net::TcpStream, SocketAddr)> {
        self.listener.accept()
    }

    #[allow(deprecated)]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.listener.set_only_v6(only_v6)
    }

    #[allow(deprecated)]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.listener.only_v6()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.listener.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.listener.ttl()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.listener.take_error()
    }
}

impl Evented for TcpListener {
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
