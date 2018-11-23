use std::mem;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use libc;

use crate::event::{Evented, EventedId, Ready};
use crate::poll::{PollCalled, PollOption, Poller};
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
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::io::{self, Read, Write};
///
/// use mio_st::unix::new_pipe;
/// use mio_st::event::{Event, Events, EventedId, Ready};
/// use mio_st::poll::{Poller, PollOption};
///
/// // Unique ids for the two ends of the channel.
/// const CHANNEL_RECV_ID: EventedId = EventedId(0);
/// const CHANNEL_SEND_ID: EventedId = EventedId(1);
///
/// // Create a `Poller` instance and the events container.
/// let mut poll = Poller::new()?;
/// let mut events = Events::new();
///
/// // Create a new pipe.
/// let (mut sender, mut receiver) = new_pipe()?;
///
/// // Register both ends of the channel.
/// poll.register(&mut receiver, CHANNEL_RECV_ID, Ready::READABLE, PollOption::Level)?;
/// poll.register(&mut sender, CHANNEL_SEND_ID, Ready::WRITABLE, PollOption::Level)?;
///
/// loop {
///     // Check for new events.
///     poll.poll(&mut events, None)?;
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

impl Evented for Receiver {
    fn register(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        self.inner.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        debug_assert!(!interests.is_writable(), "receiving end of a pipe can never be written");
        self.inner.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poller, p: PollCalled) -> io::Result<()> {
        self.inner.deregister(poll, p)
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

impl Evented for Sender {
    fn register(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        self.inner.register(poll, id, interests, opt, p)
    }

    fn reregister(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        debug_assert!(!interests.is_readable(), "sending end of a pipe can never be read");
        self.inner.reregister(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poller, p: PollCalled) -> io::Result<()> {
        self.inner.deregister(poll, p)
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
