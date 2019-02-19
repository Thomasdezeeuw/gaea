use std::mem;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::event::EventedId;
use crate::os::{Evented, Interests, PollOption, OsQueue};
use crate::sys::unix::EventedIo;

/// Create a new non-blocking unix pipe.
///
/// This is a wrapper around unix's `pipe` system call and can be used as
/// interprocess communication channel.
///
/// This channel may be created before forking the process and then one end used
/// in each process, e.g. the parent process has the sending end to send command
/// to the child process.
///
/// # Deregistering
///
/// Both `Sender` and `Receiver` will deregister themselves when dropped,
/// **iff** the file descriptors are not duplicated (via `dup(2)`).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::io::{Read, Write};
///
/// use mio_st::unix::{new_pipe, Sender, Receiver};
/// use mio_st::event::EventedId;
/// use mio_st::poll::{OsQueue, PollOption};
///
/// // Unique ids for the two ends of the channel.
/// const CHANNEL_RECV_ID: EventedId = EventedId(0);
/// const CHANNEL_SEND_ID: EventedId = EventedId(1);
///
/// // Create a `OsQueue` instance and the events container.
/// let mut poller = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// // Create a new pipe.
/// let (mut sender, mut receiver) = new_pipe()?;
///
/// // Register both ends of the channel.
/// poller.register(&mut receiver, CHANNEL_RECV_ID, Receiver::INTERESTS, PollOption::Level)?;
/// poller.register(&mut sender, CHANNEL_SEND_ID, Sender::INTERESTS, PollOption::Level)?;
///
/// loop {
///     // Check for new events.
///     poller.poll(&mut events, None)?;
///
///     for event in &mut events {
///         match event.id() {
///             CHANNEL_RECV_ID => {
///                 let mut buf = Vec::with_capacity(128);
///                 let n = receiver.read(&mut buf)?;
///                 println!("received: {:?}", &buf[0..n]);
/// #               return Ok(());
///             },
///             CHANNEL_SEND_ID => sender.write_all(b"Hello world")?,
///             _ => unreachable!(),
///         }
///     }
/// }
/// # }
/// ```
pub fn new_pipe() -> io::Result<(Sender, Receiver)> {
    let mut fds: [RawFd; 2] = unsafe { mem::uninitialized() };

    if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        for fd in &fds {
            if unsafe { libc::fcntl(*fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        let r = Receiver { inner: unsafe { EventedIo::from_raw_fd(fds[0]) } };
        let w = Sender { inner: unsafe { EventedIo::from_raw_fd(fds[1]) } };
        Ok((w, r))
    }
}

/// Receiving end of an unix pipe.
///
/// See [`new_pipe`] for documentation, including examples.
///
/// [`new_pipe`]: fn.new_pipe.html
#[derive(Debug)]
pub struct Receiver {
    inner: EventedIo,
}

impl Receiver {
    /// The interests to use when registering to receive readable events.
    pub const INTERESTS: Interests = Interests::READABLE;
}

impl Evented for Receiver {
    fn register(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        self.inner.register(poller, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        self.inner.reregister(poller, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut OsQueue) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

impl AsRawFd for Receiver {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for Receiver {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl Read for Receiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Sending end of an unix pipe.
///
/// See [`new_pipe`] for documentation, including examples.
///
/// [`new_pipe`]: fn.new_pipe.html
#[derive(Debug)]
pub struct Sender {
    inner: EventedIo,
}

impl Sender {
    /// The interests to use when registering to receive writable events.
    pub const INTERESTS: Interests = Interests::WRITABLE;
}

impl Evented for Sender {
    fn register(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        self.inner.register(poller, id, interests, opt)
    }

    fn reregister(&mut self, poller: &mut OsQueue, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        self.inner.reregister(poller, id, interests, opt)
    }

    fn deregister(&mut self, poller: &mut OsQueue) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

impl AsRawFd for Sender {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for Sender {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl Write for Sender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
