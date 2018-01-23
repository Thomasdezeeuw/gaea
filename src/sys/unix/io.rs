use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};

use event::{EventedId, Evented};
use poll::{Poll, PollOpt, Ready, Private};
use sys::unix::EventedFd;

/// Manages a file decriptor.
#[derive(Debug)]
pub struct Io {
    fd: File,
}

impl FromRawFd for Io {
    unsafe fn from_raw_fd(fd: RawFd) -> Io {
        Io { fd: File::from_raw_fd(fd) }
    }
}

impl IntoRawFd for Io {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl AsRawFd for Io {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl Evented for Io {
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

impl Read for Io {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        (&self.fd).read(dst)
    }
}

impl<'a> Read for &'a Io {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        (&self.fd).read(dst)
    }
}

impl Write for Io {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        (&self.fd).write(src)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.fd).flush()
    }
}

impl<'a> Write for &'a Io {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        (&self.fd).write(src)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.fd).flush()
    }
}
