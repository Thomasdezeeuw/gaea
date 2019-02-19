use crate::event::Event;

/// `Events` represents an events container to which events can be added.
///
/// `Events` is passed as an argument to [`Poller.poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created at the same time as a [`Poller`] and
/// reused on each call to [`Poller.poll`].
///
/// See [`Poller`] for more documentation on polling.
///
/// [`Poller.poll`]: ../poll/struct.Poller.html#method.poll
/// [`Poller`]: ../poll/struct.Poller.html
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
    /// this must return `Some`.
    fn capacity_left(&self) -> Option<usize>;

    /// Extend the events container from a slice of events.
    ///
    /// Defaults to using the [`Extend`] implementation.
    fn extend_from_slice(&mut self, events: &[Event]) {
        self.extend(events.into_iter().map(|e| *e))
    }

    /// Add a single event to the events container.
    ///
    /// Defaults to using the [`Events::extend_from_slice`] implementation.
    fn push(&mut self, event: Event) {
        self.extend_from_slice(&[event])
    }
}

impl Events for Vec<Event> {
    fn capacity_left(&self) -> Option<usize> {
        None
    }

    fn extend_from_slice(&mut self, events: &[Event]) {
        self.extend_from_slice(events);
    }

    fn push(&mut self, event: Event) {
        self.push(event);
    }
}
