use std::io;

use crate::event::EventedId;
use crate::poll::Poller;
use crate::sys;

/// Awakener allows cross-thread waking of a `Poller` instance.
///
/// When created it will cause events with [`Ready::READABLE`] and the provided
/// `id` if [`wake`] is called, possibly from another thread.
///
/// # Notes
///
/// The `Awakener` needs to be kept alive as long as wake up notifications are
/// required. This is due to an implementation detail where if all copies of the
/// `Awakener` are dropped it will also drop all wake up notifications from the
/// system queue, including wake up notifications that have been added before
/// the `Awakener` that was dropped, resulting the `Poller` instance not being
/// woken up.
///
/// Only a single `Awakener` should active per `Poller` instance, the `Awakener`
/// can be cloned using [`try_clone`] if more are needed. What happens if
/// multiple `Awakener`s are registered with the same `Poller` instance is
/// undefined.
///
/// [`Ready::READABLE`]: crate::event::Ready::READABLE
/// [`wake`]: Awakener::wake
/// [`try_clone`]: Awakener::try_clone
///
/// # Examples
///
/// Wake a `Poller` instance from another thread.
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::thread;
/// use std::time::Duration;
///
/// use mio_st::event::{Event, EventedId, Ready};
/// use mio_st::poll::{Poller, Awakener};
///
/// const WAKE_ID: EventedId = EventedId(10);
///
/// let mut poller = Poller::new()?;
/// let mut events = Vec::new();
///
/// let awakener = Awakener::new(&mut poller, WAKE_ID)?;
/// // We need to keep the Awakener alive, so we'll create a clone for the
/// // thread we create below.
/// let awakener1 = awakener.try_clone()?;
///
/// let handle = thread::spawn(move || {
///     // Working hard, or hardly working?
///     thread::sleep(Duration::from_millis(500));
///
///     // Now we'll wake the poller instance on the other thread.
///     awakener1.wake().expect("unable to wake");
/// });
///
/// // On our current thread we'll poll for events, without a timeout.
/// poller.poll(&mut events, None)?;
///
/// // After about 500 milliseconds we should we awoken by the other thread,
/// // getting a single event.
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0], Event::new(WAKE_ID, Ready::READABLE));
/// # handle.join().unwrap();
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Awakener {
    inner: sys::Awakener,
}

impl Awakener {
    /// Create a new `Awakener`.
    pub fn new(poller: &mut Poller, id: EventedId) -> io::Result<Awakener> {
        sys::Awakener::new(poller.selector(), id).map(|inner| Awakener { inner })
    }

    /// Attempts to clone the `Awakener`.
    pub fn try_clone(&self) -> io::Result<Awakener> {
        self.inner.try_clone().map(|inner| Awakener { inner })
    }

    /// Wake up the [`Poller`](Poller) instance associated with this `Awakener`.
    pub fn wake(&self) -> io::Result<()> {
        self.inner.wake()
    }
}
