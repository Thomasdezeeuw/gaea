use std::{cmp, ptr};

use arrayvec::ArrayVec;

use event::Event;

/// An iterator over a collection of readiness events.
///
/// `Events` is passed as an argument to [`Poll.poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created at the same time as a [`Poll`] and
/// reused on each call to [`Poll.poll`].
///
/// See [`Poll`] for more documentation on polling.
///
/// [`Poll.poll`]: ../struct.Poll.html#method.poll
/// [`Poll`]: ../struct.Poll.html
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use std::time::Duration;
///
/// use mio_st::event::{EventedId, Events};
/// use mio_st::poll::{Poll, PollOpt, Ready};
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::new();
///
/// // Register `Evented` handles with `poll` here.
///
/// // Run the event loop.
/// loop {
///     poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
///     for event in &mut events {
///         println!("event={:?}", event);
///     }
/// #   return Ok(());
/// }
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct Events {
    /// Stack allocted events.
    events: ArrayVec<[Event; 512]>,
    /// Position of the iterator.
    pos: usize,
}

impl Events {
    /// Create a new `Events` collection.
    ///
    /// # Notes
    ///
    /// Internally there is *currently* a maximum capacity of 512 events. At
    /// most 256 events will be used for system events.
    pub fn new() -> Events {
        Events { events: ArrayVec::new(), pos: 0 }
    }

    /// Returns the number of events in this iteration.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether or not this iteration is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Reset the events to allow it to be filled again.
    pub(crate) fn reset(&mut self) {
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
            let dst = self.events.as_mut_ptr().offset(len as isize);
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
