use std::iter::once;

use crate::event::Event;

/// `Events` represents an events container to which events can be added.
///
/// `Events` is passed as an argument to [`poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created and reused on each call to [`poll`].
///
/// See [`poll`] for more documentation on polling.
///
/// [`poll`]: fn@crate::poll
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<std::error::Error>> {
/// use std::time::Duration;
///
/// use mio_st::poll::Poller;
///
/// let mut poller = Poller::new()?;
/// // `Events` is implemented for vectors.
/// let mut events = Vec::new();
///
/// // Register `Evented` handles with `poller` here.
///
/// // Run the event loop.
/// loop {
///     poller.poll(&mut events, Some(Duration::from_millis(100)))?;
///
///     for event in &mut events {
///         println!("got event: id={:?}, rediness={:?}", event.id(), event.readiness());
///     }
/// #   return Ok(());
/// }
/// # }
/// ```
pub trait Events: Extend<Event> {
    /// Capacity left in the events container.
    ///
    /// If the container is "infinite", i.e. it can grow, this should return
    /// `None`. If there is some kind of capacity limit, e.g. in case of arrays,
    /// this must return `Some` with the available capacity left, **not total
    /// capacity**.
    ///
    /// # Notes
    ///
    /// If this returns `Some` and the capacity left is incorrect it will cause
    /// missing events.
    fn capacity_left(&self) -> Option<usize>;

    /// Add a single event to the events container.
    ///
    /// Defaults to using the [`Events::extend_from_slice`] implementation.
    fn push(&mut self, event: Event) {
        self.extend(once(event))
    }
}

impl Events for Vec<Event> {
    fn capacity_left(&self) -> Option<usize> {
        None
    }

    fn push(&mut self, event: Event) {
        self.push(event);
    }
}
