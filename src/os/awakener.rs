use std::io;

use crate::{event, sys};
use crate::os::OsQueue;

/// Awakener allows cross-thread waking of [`OsQueue`].
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
/// the `Awakener` that was dropped, resulting the [`OsQueue`] not being woken
/// up.
///
/// Only a single `Awakener` should active per [`OsQueue`], the `Awakener` can
/// be cloned using [`try_clone`] if more are needed. What happens if multiple
/// `Awakener`s are registered with the same `OsQueue` is undefined.
///
/// [`Ready::READABLE`]: crate::event::Ready::READABLE
/// [`wake`]: Awakener::wake
/// [`try_clone`]: Awakener::try_clone
///
/// # Examples
///
/// Wake an [`OsQueue`] from another thread.
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::io;
/// use std::thread;
/// use std::time::Duration;
///
/// use mio_st::{event, poll};
/// use mio_st::event::{Event, Ready};
/// use mio_st::os::{OsQueue, Awakener};
///
/// const WAKE_ID: event::Id = event::Id(10);
///
/// let mut os_queue = OsQueue::new()?;
/// let mut events = Vec::new();
///
/// let awakener = Awakener::new(&mut os_queue, WAKE_ID)?;
///
/// // We need to keep the Awakener alive, so we'll create a clone for the
/// // thread we create below.
/// let awakener1 = awakener.try_clone()?;
/// let handle = thread::spawn(move || {
///     // Working hard, or hardly working?
///     thread::sleep(Duration::from_millis(500));
///
///     // Now we'll wake the queue on the other thread.
///     awakener1.wake().expect("unable to wake");
/// });
///
/// // On our current thread we'll poll for events, without a timeout.
/// poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
///
/// // After about 500 milliseconds we should we awoken by the other thread we
/// // started, getting a single event.
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
    pub fn new(os_queue: &mut OsQueue, id: event::Id) -> io::Result<Awakener> {
        sys::Awakener::new(os_queue.selector(), id).map(|inner| Awakener { inner })
    }

    /// Attempts to clone the `Awakener`.
    pub fn try_clone(&self) -> io::Result<Awakener> {
        self.inner.try_clone().map(|inner| Awakener { inner })
    }

    /// Wake up the [`OsQueue`] associated with this `Awakener`.
    pub fn wake(&self) -> io::Result<()> {
        self.inner.wake()
    }
}
