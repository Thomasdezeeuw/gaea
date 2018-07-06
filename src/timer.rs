//! Timer utilities.

use std::io;
use std::time::{Duration, Instant};

use event::{Evented, EventedId, Ready, INVALID_EVENTED_ID};
use poll::{PollCalled, PollOption, Poller};

/// A timer that can be registered with [`Poller`].
///
/// Timers can be created using a specific [`deadline`] or using a [`timeout`].
/// When registering the timer it will trigger an [`Event`] after the deadline
/// has passed, which gets returned when [polling]. The `Event` will always have
/// [`TIMER`] as `Ready` value and the same `id` as provided when registering
/// it.
///
/// [`Poller`]: ../poll/struct.Poller.html
/// [`deadline`]: #method.deadline
/// [`timeout`]: #method.timeout
/// [`Event`]: ../event/struct.Event.html
/// [polling]: ../poll/struct.Poller.html#method.poll
/// [`TIMER`]: ../event/struct.Ready.html#associatedconstant.TIMER
///
/// # Panics
///
/// When (re)registering a `Timer` the interests must always be [`Ready::TIMER`]
/// and the poll option [`PollOption::Oneshot`], those methods will panic
/// otherwise. This is required because those are the only events `Timer`s can
/// currently create, allowing anything else would be confusing.
///
/// [`Ready::TIMER`]: ../event/struct.Ready.html#associatedconstant.TIMER
/// [`PollOption::Oneshot`]: ../poll/enum.PollOption.html#variant.Oneshot
///
/// # Notes
///
/// Deregistering a `Timer` is a costly operation. For better performance it is
/// advised to not bothering with deregistering and instead ignore the event
/// when it comes up. Deregistering also returns `Ok(())` even when the deadline
/// was not found, as it was already triggered. This is because the goal of
/// deregistering is to prevent further events, which still holds true event if
/// the timer is not found (and thus not removed).
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{Event, Events, EventedId, Ready};
/// use mio_st::poll::{Poller, PollOption};
/// use mio_st::timer::Timer;
///
/// // Our `Poller` instance and events.
/// let mut poll = Poller::new()?;
/// let mut events = Events::new();
///
/// // Create our timer, with a deadline 10 milliseconds from now.
/// let mut timer = Timer::timeout(Duration::from_millis(10));
///
/// // Register our timer with our `Poller` instance. Note that both
/// // `Ready::TIMER` and `PollOption::Oneshot` are required when registering a
/// // `Timer`. See Panics section above.
/// poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Oneshot)?;
///
/// // Even though we don't provide a timeout to poll this will return in
/// // roughly 10 milliseconds and return an event with our deadline.
/// poll.poll(&mut events, None)?;
///
/// assert_eq!(events.len(), 1);
/// for event in &mut events {
///     assert_eq!(event, Event::new(EventedId(0), Ready::TIMER));
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
    id: EventedId,
    deadline: Instant,
}

impl Timer {
    /// Create a new `Timer` based on a deadline.
    pub fn deadline(deadline: Instant) -> Timer {
        Timer { id: INVALID_EVENTED_ID, deadline }
    }

    /// Create a new `Timer` based on a timeout.
    ///
    /// This shorthand for `Timer::deadline(Instant::now() + timeout)`.
    pub fn timeout(timeout: Duration) -> Timer {
        Timer::deadline(Instant::now() + timeout)
    }
}

impl Evented for Timer {
    fn register(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, _: PollCalled) -> io::Result<()> {
        debug_assert_eq!(interests, Ready::TIMER, "trying to (re)register `Timer` with interests other then `TIMER`");
        debug_assert_eq!(opt, PollOption::Oneshot, "trying to (re)register `Timer` with poll option other then `Oneshot`");
        self.id = id;
        poll.add_deadline(id, self.deadline)
    }

    fn reregister(&mut self, poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, p: PollCalled) -> io::Result<()> {
        self.register(poll, id, interests, opt, p)
    }

    fn deregister(&mut self, poll: &mut Poller, _: PollCalled) -> io::Result<()> {
        poll.remove_deadline(self.id)
    }
}
