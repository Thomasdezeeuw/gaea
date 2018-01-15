use std::io;
use std::time::{Duration, Instant};

use event::Evented;
use poll::{Poll, PollOpt, Ready, Token, INVALID_TOKEN};

/// Timer provides a way to creates events based on time.
///
/// # Registering
///
/// When registering a `Timer` the `Ready` and `PollOpt` arguments are ignored;
/// it only fires once and `Ready` will always be [timeout].
///
/// Deregistering is not needed if the deadline has been passed, like stated
/// above it's only fired once and forgotten after that.
///
/// [timeout]: ../poll/struct.Ready.html#associatedconstant.TIMEOUT
///
/// # Reuse
///
/// A `Timer` can be reused, although it has very little value.
///
/// Note that calling `deregister` after a second call to `register` or
/// `reregister`, will not deregister the first registration of the timer.
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio::poll::{Poll, Token, Ready, PollOpt};
/// use mio::event::{Event, Events};
/// use mio::timer::Timer;
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(1024);
///
/// // Create and register our timer.
/// let mut timer = Timer::timeout(Duration::from_millis(10));
/// poll.register(&mut timer, Token(0), Ready::TIMEOUT, PollOpt::ONESHOT)?;
///
/// poll.poll(&mut events, None)?;
///
/// for event in &mut events {
///     assert_eq!(event, Event::new(Ready::TIMEOUT, Token(0)));
/// }
/// #   Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
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
