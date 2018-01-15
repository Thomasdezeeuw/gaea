use std::io;
use std::time::{Duration, Instant};

use {Ready, Token};
use event::Evented;
use poll::{Poll, PollOpt, INVALID_TOKEN};

/// Timer provides a way to creates events based on time.
///
/// # Registering
///
/// When registering a `Timer` the `Ready` and `PollOpt` arguments are ignored;
/// it only fires once and `Ready` will always be both readable and writable.
///
/// Deregistering is not needed if the deadline has been passed, like stated
/// above it's only fired once and forgotten after that.
///
/// # Reuse
///
/// A `Timer` can be reused, although it has very little value. Note that
/// calling `deregister` after a second call to `register` or `reregister`, will
/// not deregister the first timer.
#[derive(Debug)]
pub struct Timer {
    deadline: Instant,
    token: Token,
}

impl Timer {
    /// Create a new Timer with the provided `deadline`.
    pub fn new(deadline: Instant) -> Timer {
        Timer { deadline, token: INVALID_TOKEN }
    }

    /// Create a new Timer with the deadline set to `now + timeout`.
    pub fn timeout(timeout: Duration) -> Timer {
        Timer { deadline: Instant::now() + timeout, token: INVALID_TOKEN }
    }

    /// Returns the deadline of this Timer.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl Evented for Timer {
    fn register(&mut self, poll: &mut Poll, token: Token, _: Ready, _: PollOpt) -> io::Result<()> {
        self.token = token;
        poll.add_deadline(token, self.deadline);
        Ok(())
    }
    fn reregister(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.register(poll, token, interest, opts)
    }
    fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
        if self.token != INVALID_TOKEN {
            poll.remove_deadline(self.token);
        }
        Ok(())
    }
}
