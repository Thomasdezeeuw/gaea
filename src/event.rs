//! Readiness event types.

use core::fmt;
use core::ops::{BitOr, BitOrAssign};
use core::time::Duration;

/// A readiness event source that can be polled for events.
///
/// The trait has a generic parameter `Evts` which must implement [`Events`],
/// this should always be a generic parameter.
///
/// # Implementing event source
///
/// The trait has two generic parameters: `Evts` and `E`. `Evts` should remain
/// generic to support all types of events containers. `E` should also be
/// generic but a trait bound `From<MyError>` should be added, this way [`poll`]
/// can return a single error from multiple event sources. The example below
/// shows how this works.
///
/// [`poll`]: crate::poll
///
/// ```
/// use std::time::Duration;
///
/// use mio_st::{event, Events, Event, poll};
///
/// /// Our event source that implements `event::Source`.
/// struct MyEventSource(Vec<Event>);
///
/// /// The error returned by our even source implementation.
/// struct MyError;
///
/// impl<Evts, E> event::Source<Evts, E> for MyEventSource
///     where Evts: Events, // We keep the events container generic to support
///                         // all kinds of events containers.
///           E: From<MyError>, // We add this bound to allow use to convert
///                             // `MyError` into the generic error `E`.
/// {
///     fn next_event_available(&self) -> Option<Duration> {
///         if !self.0.is_empty() {
///             // If we have an event ready we don't want to block.
///             Some(Duration::from_millis(0))
///         } else {
///             // If we don't have any events we don't have a preference about
///             // blocking times.
///             None
///         }
///     }
///
///     fn poll(&mut self, events: &mut Evts) -> Result<(), E> {
///         match poll_events(events) {
///             Ok(()) => Ok(()),
///             // We need explicitly call `into()` to convert our error into
///             // the generic error. Note that this isn't required when using
///             // `?` (the try operator).
///             Err(err) => Err(err.into()),
///         }
///     }
/// }
/// # fn poll_events<Evts>(_events: &mut Evts) -> Result<(), MyError> { Ok(()) }
///
/// // Implementing `From` for `()` allows us to use it as an error in our call
/// // to poll below..
/// impl From<MyError> for () {
///     fn from(_err: MyError) -> () {
///         ()
///     }
/// }
///
/// # fn main() -> Result<(), ()> {
/// // Now can use our event source with `()` as error type.
/// let mut my_source = MyEventSource(Vec::new());
/// let mut events = Vec::new();
/// poll::<_, ()>(&mut [&mut my_source], &mut events, None)?;
/// # Ok(())
/// # }
/// ```
pub trait Source<Evts, E>
    where Evts: Events,
{
    /// The duration until the next event will be available.
    ///
    /// This is used to determine what timeout to use in a [blocking call] to
    /// poll. For example if we have a queue of timers, of which the next one
    /// expires in one second, we don't want to block for more then one second
    /// and thus we should return `Some(1 second)` to ensure that.
    ///
    /// If the duration until the next available event is unknown `None` should
    /// be returned.
    ///
    /// [blocking call]: Source::blocking_poll
    fn next_event_available(&self) -> Option<Duration>;

    /// Poll for events.
    ///
    /// Any available readiness events must be added to `events`. The pollable
    /// source may not block.
    ///
    /// Some implementation of [`Events`] have a limited available capacity.
    /// This method may not add more events then [`Events::capacity_left`]
    /// returns, if it returns a capacity limit. Available events that don't fit
    /// in the events container in a single call to poll should remain in the
    /// source and should be added to the events container in future calls to
    /// poll.
    fn poll(&mut self, events: &mut Evts) -> Result<(), E>;

    /// A blocking poll for readiness events.
    ///
    /// This is the same as [`Source::poll`] and all requirements of that method
    /// apply to this method as well. Different to `poll` is that this method
    /// may block up to `timeout` duration, if one is provided, or block forever
    /// if no timeout is provided (assuming *something* wakes up the poll
    /// source).
    ///
    /// The default implementation simply calls `poll`, thus it doesn't actually
    /// block.
    fn blocking_poll(&mut self, events: &mut Evts, _timeout: Option<Duration>) -> Result<(), E> {
        self.poll(events)
    }
}

impl<S, Evts, E> Source<Evts, E> for &mut S
    where S: Source<Evts, E>,
          Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        (&**self).next_event_available()
    }

    fn poll(&mut self, events: &mut Evts) -> Result<(), E> {
        (&mut **self).poll(events)
    }

    fn blocking_poll(&mut self, events: &mut Evts, timeout: Option<Duration>) -> Result<(), E> {
        (&mut **self).blocking_poll(events, timeout)
    }
}

/// `Events` represents an events container to which events can be added.
///
/// `Events` is passed as an argument to [`poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created and reused on each call to [`poll`].
///
/// See [`poll`] for more documentation on polling.
///
/// [`poll`]: crate::poll
///
/// # Why a trait?
///
/// A possible question that might arise is: "why is `Events` a trait and not a
/// concrete type?" The answer is flexibility. Previously `Events` was a vector,
/// but most users actually have there own data structure with runnable
/// processes, green threads, `Future`s, etc. This meant that `Events` was often
/// an intermediate storage used to receive events only to mark processes as
/// runnable and run them later.
///
/// Using a trait removes the need for this intermediate storage and allows
/// users to direct mark processes as runnable inside there own data structure.
///
/// # Examples
///
/// An implementation of `Events` for an array.
///
/// ```
/// # fn main() -> Result<(), ()> {
/// use mio_st::{event, Events, Event, Queue, Ready, poll};
///
/// const EVENTS_SIZE: usize = 32;
///
/// /// Our `Events` implementation.
/// struct MyEvents([Option<Event>; EVENTS_SIZE]);
///
/// impl Events for MyEvents {
///     fn capacity_left(&self) -> event::Capacity {
///         let limit = self.0.iter().position(Option::is_some).unwrap_or(EVENTS_SIZE);
///         event::Capacity::Limited(limit)
///     }
///
///     fn add(&mut self, event: Event) {
///         let index = self.0.iter().position(Option::is_none).unwrap();
///         self.0[index] = Some(event);
///     }
/// }
///
/// // An event source, with some events.
/// let mut queue = Queue::new();
/// let event1 = Event::new(event::Id(0), Ready::READABLE);
/// queue.add(event1);
/// let event2 = Event::new(event::Id(1), Ready::WRITABLE);
/// queue.add(event2);
///
/// // Poll the source.
/// let mut events = MyEvents([None; EVENTS_SIZE]);
/// poll(&mut [&mut queue], &mut events, None)?;
/// assert_eq!(events.0[0], Some(event1));
/// assert_eq!(events.0[1], Some(event2));
/// # Ok(())
/// # }
/// ```
pub trait Events {
    /// Capacity left in the events container.
    ///
    /// This must return the available capacity left, **not total capacity**.
    ///
    /// # Notes
    ///
    /// If this returns [`Capacity::Growable`] and the capacity left is
    /// incorrect it may cause missing events.
    fn capacity_left(&self) -> Capacity;

    /// Add a single event to the events container.
    fn add(&mut self, event: Event);

    /// Extend the events containers with multiple events.
    fn extend<I>(&mut self, events: I)
        where I: Iterator<Item = Event>,
    {
        for event in events {
            self.add(event);
        }
    }
}

impl<'a, Evts> Events for &'a mut Evts
    where Evts: Events,
          &'a mut Evts: Extend<Event>,
{
    fn capacity_left(&self) -> Capacity {
        (&**self).capacity_left()
    }

    fn add(&mut self, event: Event) {
        (&mut **self).add(event)
    }

    fn extend<I>(&mut self, events: I)
        where I: Iterator<Item = Event>,
    {
        (&mut **self).extend(events)
    }
}

#[cfg(feature = "std")]
impl Events for Vec<Event> {
    fn capacity_left(&self) -> Capacity {
        Capacity::Growable
    }

    fn add(&mut self, event: Event) {
        self.push(event);
    }

    fn extend<I>(&mut self, events: I)
        where I: Iterator<Item = Event>,
    {
        <Self as Extend<Event>>::extend(self, events);
    }
}

/// The capacity left in the events container.
///
/// If the container is "infinite", i.e. it can grow, it should use `Growable`.
/// If there is some kind of capacity limit `Limited` should be used.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Capacity {
    /// The capacity is limited.
    Limited(usize),
    /// The events container is growable making the container capacity
    /// "infinite". This is for example return in the [`Events`] implements for
    /// vectors.
    Growable,
}

impl Capacity {
    /// Get the maximum capacity given the events container's capacity and the
    /// number of available events.
    ///
    /// # Examples
    ///
    /// For event contains without a capacity limit it will always return
    /// `right`.
    ///
    /// ```
    /// use mio_st::Events;
    ///
    /// let n_events = 5;
    /// let events = Vec::new();
    /// assert_eq!(events.capacity_left().min(n_events), 5);
    /// ```
    ///
    /// For limit capacity events containers to will take the `min`imum value.
    ///
    /// ```
    /// use mio_st::{event, Events, Event, Ready};
    ///
    /// struct MyEventContainer(Option<Event>);
    ///
    /// impl Events for MyEventContainer {
    ///     fn capacity_left(&self) -> event::Capacity {
    ///         if self.0.is_some() {
    ///             event::Capacity::Limited(0)
    ///         } else {
    ///             event::Capacity::Limited(1)
    ///         }
    ///     }
    ///
    ///     fn add(&mut self, event: Event) {
    ///         self.0 = Some(event);
    ///     }
    /// }
    ///
    /// let n_events = 5;
    /// let events = MyEventContainer(None);
    /// assert_eq!(events.capacity_left().min(n_events), 1);
    ///
    /// let events = MyEventContainer(Some(Event::new(event::Id(0), Ready::READABLE)));
    /// assert_eq!(events.capacity_left().min(n_events), 0);
    /// ```
    pub fn min(self, right: usize) -> usize {
        match self {
            Capacity::Limited(left) => left.min(right),
            Capacity::Growable => right,
        }
    }
}

/// A readiness event.
///
/// `Event` is a [readiness state] paired with an [id]. Events are returned by
/// [`poll`].
///
/// For more documentation on polling and events, see [`poll`].
///
/// [readiness state]: Ready
/// [id]: Id
/// [`poll`]: crate::poll
///
/// # Examples
///
/// ```
/// use mio_st::{event, Event, Ready};
///
/// let my_event = Event::new(event::Id(0), Ready::READABLE | Ready::WRITABLE);
///
/// assert_eq!(my_event.id(), event::Id(0));
/// assert_eq!(my_event.readiness(), Ready::READABLE | Ready::WRITABLE);
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Event {
    id: Id,
    readiness: Ready,
}

impl Event {
    /// Creates a new `Event` containing `id` and `readiness`.
    pub const fn new(id: Id, readiness: Ready) -> Event {
        Event { id, readiness }
    }

    /// Returns the event's id.
    pub const fn id(&self) -> Id {
        self.id
    }

    /// Returns the event's readiness.
    pub const fn readiness(&self) -> Ready {
        self.readiness
    }
}

/// Identifier of an event.
///
/// This is used to associate a readiness notifications with event handle.
///
/// See [`poll`] for more documentation on polling.
///
/// [`poll`]: crate::poll
///
/// # Uniqueness of `Id`
///
/// `Id` does not have to be unique, it is purely a tool for the user to
/// associate an `Event` with an event handle. It is advised for example to use
/// the same `Id` for say a `TcpStream` and any related timeout or deadline for
/// the same connection. The `Id` is effectively opaque to any readiness event
/// sources.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Id(pub usize);

impl From<usize> for Id {
    fn from(val: usize) -> Id {
        Id(val)
    }
}

impl From<Id> for usize {
    fn from(val: Id) -> usize {
        val.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A set of readiness event kinds.
///
/// `Ready` is a set of operation descriptors indicating which kind of operation
/// is ready to be performed. For example, `Ready::READABLE` indicates that the
/// associated `Evented` handle is ready to perform a read operation.
///
/// `Ready` values can be combined together using the various bitwise operators,
/// see examples below.
///
/// For high level documentation on polling and readiness, see [`poll`].
///
/// [`poll`]: crate::poll
///
/// # Examples
///
/// ```
/// use mio_st::Ready;
///
/// let ready = Ready::READABLE | Ready::WRITABLE;
///
/// assert!(ready.is_readable());
/// assert!(ready.is_writable());
/// assert!(!ready.is_error());
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Ready(u8);

const READABLE: u8 = 1;
const WRITABLE: u8 = 1 << 1;
const ERROR: u8 = 1 << 2;
const TIMER: u8 = 1 << 3;
#[cfg(unix)]
const HUP: u8 = 1 << 4;

impl Ready {
    /// Empty set.
    pub const EMPTY: Ready = Ready(0);

    /// Readable readiness.
    pub const READABLE: Ready = Ready(READABLE);

    /// Writable readiness.
    pub const WRITABLE: Ready = Ready(WRITABLE);

    /// Error readiness.
    pub const ERROR: Ready = Ready(ERROR);

    /// Deadline was elapsed.
    pub const TIMER: Ready = Ready(TIMER);

    /// Hup readiness, this signal is Unix specific.
    #[cfg(unix)]
    pub const HUP: Ready = Ready(HUP);

    /// Whether or not all flags in `other` are contained within `self`.
    #[inline]
    pub const fn contains(self, other: Ready) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if the value includes readable readiness.
    #[inline]
    pub const fn is_readable(self) -> bool {
        self.contains(Self::READABLE)
    }

    /// Returns true if the value includes writable readiness.
    #[inline]
    pub const fn is_writable(self) -> bool {
        self.contains(Self::WRITABLE)
    }

    /// Returns true if the value includes error readiness.
    #[inline]
    pub const fn is_error(self) -> bool {
        self.contains(Self::ERROR)
    }

    /// Returns true if a deadline has elapsed.
    #[inline]
    pub const fn is_timer(self) -> bool {
        self.contains(Self::TIMER)
    }

    /// Returns true if the value includes HUP readiness.
    #[inline]
    #[cfg(unix)]
    pub const fn is_hup(self) -> bool {
        self.contains(Self::HUP)
    }
}

impl BitOr for Ready {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Ready(self.0 | rhs.0)
    }
}

impl BitOrAssign for Ready {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

macro_rules! fmt_debug {
    ($self:expr, $f:expr, $($flag:expr),+) => {{
        if $self.0 == 0 {
            $f.write_str("(empty)")
        } else {
            let mut first = true;
            $(
            if $self.0 & $flag != 0 {
                if !first {
                    $f.write_str(" | ")?;
                } else {
                    first = false;
                }
                $f.write_str(stringify!($flag))?;
            }
            )+

            // This is silly but it is to circumvent a `unused_assignments`
            // warning for the last write to `first`.
            #[allow(clippy::drop_copy)]
            drop(first);

            Ok(())
        }
    }}
}

impl fmt::Debug for Ready {
    #[allow(clippy::cognitive_complexity)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_debug!(self, f, READABLE, WRITABLE, ERROR, TIMER, HUP)
    }
}
