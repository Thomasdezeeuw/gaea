use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::event::EventedId;
use crate::os::{Evented, Interests, PollOption, OsQueue};
use crate::sys::unix::EventedFd;

/// Managed adaptor for a `RawFd` providing an [`Evented`] implementation.
///
/// Where [`EventedFd`] does not take ownership of the file descriptor,
/// `EventedIo` does. It will manage the lifecycle of the file descriptor, such
/// as closing it when dropped. Otherwise `EventedFd` and `EventedIo` are much
/// alike, since this uses `EventedFd` internally.
///
/// You could see `EventedIo` as an owned file descriptor, while `EventedIo` is
/// a borrowed file descriptor.
///
/// `EventedIo` can be created by calling `from_raw_fd`, see the examples below.
///
/// # Deregistering
///
/// `EventedIo` will deregister itself when dropped, **iff** the file descriptor
/// is unique (i.e. it is not duplicated via `dup(2)`).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::net::TcpListener;
/// use std::os::unix::io::{FromRawFd, IntoRawFd};
///
/// use mio_st::event::EventedId;
/// use mio_st::poll::{Interests, PollOption, OsQueue};
/// use mio_st::unix::EventedIo;
///
/// // Bind a listener from the standard library.
/// let listener = TcpListener::bind("127.0.0.1:0")?;
///
/// // Turn the into it's file descriptor. Note the use of **into**_raw_fd here,
/// // not **as**_raw_fd, because `EventedIo` will manage the lifetime for us.
/// let listener_fd = listener.into_raw_fd();
///
/// // Now we can let `EventedIo` manage the lifetime for us.
/// let mut evented_listener = unsafe { EventedIo::from_raw_fd(listener_fd) };
///
/// let mut poller = OsQueue::new()?;
///
/// // Register the listener using `EventedFd`.
/// poller.register(&mut evented_listener, EventedId(0), Interests::READABLE, PollOption::Edge)?;
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct EventedIo {
    fd: File,
}

impl FromRawFd for EventedIo {
    unsafe fn from_raw_fd(fd: RawFd) -> EventedIo {
        EventedIo { fd: File::from_raw_fd(fd) }
    }
}

impl IntoRawFd for EventedIo {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl AsRawFd for EventedIo {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl Evented for EventedIo {
    fn register(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poller, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poller, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut OsQueue) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poller)
    }
}

impl Read for EventedIo {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        (&self.fd).read(dst)
    }
}

impl Write for EventedIo {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        (&self.fd).write(src)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.fd).flush()
    }
}
