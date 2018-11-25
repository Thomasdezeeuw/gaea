use std::{cmp, ptr};
use std::iter::FusedIterator;

use arrayvec::ArrayVec;

use crate::event::Event;

/// An iterator over a collection of readiness events.
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
/// use mio_st::event::{EventedId, Events, Ready};
/// use mio_st::poll::{Poller, PollOption};
///
/// let mut poller = Poller::new()?;
/// let mut events = Events::new();
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
#[derive(Debug)]
pub struct Events {
    /// Stack allocated events.
    events: ArrayVec<[Event; 256]>,
    /// Position of the iterator.
    pos: usize,
}

impl Events {
    /// Create a new `Events` collection.
    ///
    /// # Notes
    ///
    /// Internally there is *currently* a maximum capacity of 256 events. At
    /// most 128 events will be used for system events.
    pub fn new() -> Events {
        Events { events: ArrayVec::new(), pos: 0 }
    }

    /// Returns the number of events in this iteration.
    pub fn len(&self) -> usize {
        let len = self.events.len();
        if self.pos > len {
            0
        } else {
            len - self.pos
        }
    }

    /// Whether or not this iteration is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the events to allow it to be filled again.
    pub(crate) fn clear(&mut self) {
        // TODO: Use `events.clear` in the future: see
        // https://github.com/bluss/arrayvec/pull/98.
        // This is safe because `Event` doesn't implement `Drop`.
        unsafe { self.events.set_len(0); }
        self.pos = 0;
    }

    /// Returns the capacity.
    pub(crate) fn capacity(&self) -> usize {
        self.events.capacity()
    }

    /// Add an user space event.
    pub(crate) fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Extend the events, returns the number of events added.
    pub(crate) fn extend_events(&mut self, events: &[Event]) -> usize {
        let count = cmp::min(self.capacity_left(), events.len());
        if count == 0 {
            return 0;
        }

        let len = self.len();
        unsafe {
            let dst = self.events.as_mut_ptr().add(len);
            ptr::copy_nonoverlapping(events.as_ptr(), dst, count);
            self.events.set_len(len + count);
        }

        count
    }

    /// Returns the leftover capacity.
    pub(crate) fn capacity_left(&self) -> usize {
        self.events.capacity() - self.events.len()
    }
}

impl Default for Events {
    fn default() -> Events {
        Events::new()
    }
}

impl<'a> Iterator for &'a mut Events {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let ret = self.events.get(self.pos).cloned();
        self.pos += 1;
        ret
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for &'a mut Events {
    fn len(&self) -> usize {
        // & &mut self -> & self.
        (&**self).len()
    }
}

impl<'a> FusedIterator for &'a mut Events { }

#[cfg(test)]
mod tests {
    use std::iter::repeat;

    use crate::event::{Events, Event, EventedId, Ready};

    const EVENTS_CAP: usize = 256;

    #[test]
    fn push() {
        let mut events = Events::default();
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP);
        assert_eq!((&mut events).len(), 0); // ExactSizeIterator implementation.

        events.push(Event::new(EventedId(0), Ready::all()));
        assert_eq!(events.len(), 1);
        assert!(!events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP - 1);
        assert_eq!((&mut events).len(), 1);
    }

    #[test]
    fn clear() {
        let mut events = Events::new();
        events.push(Event::new(EventedId(0), Ready::all()));

        events.clear();
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP);
    }

    #[test]
    fn extend_0() {
        let mut events = Events::new();

        assert_eq!(events.extend_events(&[]), 0);
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP);
        assert_eq!((&mut events).len(), 0);
    }

    #[test]
    fn extend_many() {
        let extra_events: [Event; 4] = [
            Event::new(EventedId(0), Ready::all()),
            Event::new(EventedId(1), Ready::all()),
            Event::new(EventedId(2), Ready::all()),
            Event::new(EventedId(3), Ready::all()),
        ];

        let mut events = Events::new();
        assert_eq!(events.extend_events(&extra_events), 4);
        assert_eq!(events.len(), 4);
        assert!(!events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP - 4);
        assert_eq!((&mut events).len(), 4);
    }

    #[test]
    fn extend_no_space() {
        let iter = repeat(Event::new(EventedId(0), Ready::all()));
        let extra_events: Vec<Event> = iter.take(EVENTS_CAP + 1).collect();

        let mut events = Events::new();
        assert_eq!(events.extend_events(&extra_events), EVENTS_CAP);
        assert_eq!(events.len(), EVENTS_CAP);
        assert!(!events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), 0);
        assert_eq!((&mut events).len(), EVENTS_CAP);

        assert_eq!(events.extend_events(&extra_events), 0);
    }

    #[test]
    fn iter() {
        let extra_events: [Event; 4] = [
            Event::new(EventedId(0), Ready::all()),
            Event::new(EventedId(1), Ready::all()),
            Event::new(EventedId(2), Ready::all()),
            Event::new(EventedId(3), Ready::all()),
        ];

        let mut events = Events::new();
        assert_eq!(events.extend_events(&extra_events), 4);

        let mut current_id = 0;
        for event in &mut events.take(2) {
            assert_eq!(event.id(), EventedId(current_id));
            assert_eq!(event.readiness(), Ready::all());
            current_id += 1;
        }

        assert_eq!(events.len(), 2);
        assert!(!events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP - 4);
        assert_eq!((&mut events).len(), 2);

        for event in &mut events {
            assert_eq!(event.id(), EventedId(current_id));
            assert_eq!(event.readiness(), Ready::all());
            current_id += 1;
        }

        assert_eq!(current_id, 4);
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP - 4);
        assert_eq!((&mut events).len(), 0);
    }

    #[test]
    fn fused_iter() {
        let mut events = Events::new();
        events.push(Event::new(EventedId(0), Ready::all()));

        assert_eq!((&mut events).count(), 1);
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
        assert_eq!(events.capacity(), EVENTS_CAP);
        assert_eq!(events.capacity_left(), EVENTS_CAP - 1);
        assert_eq!((&mut events).len(), 0);

        for _ in 0..100 {
            assert_eq!((&mut events).next(), None);
        }
    }
}
