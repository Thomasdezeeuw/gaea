use std::fs::File;
use std::io::{self, Read, Write};
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::event;
use crate::os::{Evented, Interests, PollOption, OsQueue};
use crate::sys::unix::EventedFd;

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
/// use mio_st::os::{OsQueue, PollOption};
/// use mio_st::unix::{new_pipe, Sender, Receiver};
/// use mio_st::{event, poll};
///
/// // Unique ids for the two ends of the channel.
/// const CHANNEL_RECV_ID: event::Id = event::Id(0);
/// const CHANNEL_SEND_ID: event::Id = event::Id(1);
///
/// // Create a `OsQueue` and the events container.
/// let mut os_queue = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// // Create a new pipe.
/// let (mut sender, mut receiver) = new_pipe()?;
///
/// // Register both ends of the channel.
/// os_queue.register(&mut receiver, CHANNEL_RECV_ID, Receiver::INTERESTS, PollOption::LEVEL)?;
/// os_queue.register(&mut sender, CHANNEL_SEND_ID, Sender::INTERESTS, PollOption::LEVEL)?;
///
/// const MSG: &[u8; 11] = b"Hello world";
///
/// loop {
///     // Poll for events.
///     poll(&mut os_queue, &mut [], &mut events, None)?;
///
///     for event in &mut events {
///         match event.id() {
///             CHANNEL_SEND_ID => sender.write_all(MSG)?,
///             CHANNEL_RECV_ID => {
///                 let mut buf = [0; 11];
///                 let n = receiver.read(&mut buf)?;
///                 println!("received: {:?}", &buf[0..n]);
/// #               assert_eq!(n, MSG.len());
/// #               assert_eq!(&buf, &*MSG);
/// #               return Ok(());
///             },
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
        let r = Receiver { inner: unsafe { File::from_raw_fd(fds[0]) } };
        let w = Sender { inner: unsafe { File::from_raw_fd(fds[1]) } };
        Ok((w, r))
    }
}

/// Receiving end of an unix pipe.
///
/// See [`new_pipe`] for documentation, including examples.
#[derive(Debug)]
pub struct Receiver {
    inner: File,
}

impl Receiver {
    /// The interests to use when registering to receive readable events.
    pub const INTERESTS: Interests = Interests::READABLE;
}

impl Evented for Receiver {
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        EventedFd(&self.inner.as_raw_fd()).register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        EventedFd(&self.inner.as_raw_fd()).reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        EventedFd(&self.inner.as_raw_fd()).deregister(os_queue)
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
#[derive(Debug)]
pub struct Sender {
    inner: File,
}

impl Sender {
    /// The interests to use when registering to receive writable events.
    pub const INTERESTS: Interests = Interests::WRITABLE;
}

impl Evented for Sender {
    fn register(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        EventedFd(&self.inner.as_raw_fd()).register(os_queue, id, interests, opt)
    }

    fn reregister(&mut self, os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        EventedFd(&self.inner.as_raw_fd()).reregister(os_queue, id, interests, opt)
    }

    fn deregister(&mut self, os_queue: &mut OsQueue) -> io::Result<()> {
        EventedFd(&self.inner.as_raw_fd()).deregister(os_queue)
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
