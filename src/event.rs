//! Readiness event types.

use std::iter::once;
use std::ops::{BitOr, BitOrAssign};
use std::time::Duration;
use std::{fmt, io};

/// A readiness event source that can be polled for readiness events.
pub trait Source<Evts>
    where Evts: Events,
{
    /// The duration until the next event will be available.
    ///
    /// This is used to determine what timeout to use in a blocking call to
    /// poll. For example if we have a queue of timers, of which the next one
    /// expires in one second, we don't want to block for more then one second
    /// and thus we should return `Some(1 second)` to ensure that.
    ///
    /// If the duration until the next available event is unknown `None` should
    /// be returned.
    fn next_event_available(&self) -> Option<Duration>;

    /// Poll for events.
    ///
    /// Any available readiness events must be added to `events`. The pollable
    /// source may not block.
    ///
    /// Some implementation of `Events` have a limited available capacity.
    /// This method may not add more events then `Events::capacity_left`
    /// returns, if it returns a capacity limit.
    fn poll(&mut self, events: &mut Evts) -> io::Result<()>;
}

/// A blocking variant of [`Source`].
pub trait BlockingSource<Evts>: Source<Evts>
    where Evts: Events,
{
    /// A blocking poll for readiness events.
    ///
    /// This is the same as [`Source::poll`] and all requirements of that method
    /// apply to this method as well. Different to `poll` is that this method
    /// may block up `timeout` duration, if one is provided, or block forever if
    /// no timeout is provided (assuming *something* wakes up the poll source).
    fn blocking_poll(&mut self, events: &mut Evts, timeout: Option<Duration>) -> io::Result<()>;
}

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

/// A readiness event.
///
/// `Event` is a [readiness state] paired with a [`Id`]. It is returned by
/// [`poll`].
///
/// For more documentation on polling and events, see [`poll`].
///
/// [readiness state]: Ready
/// [`poll`]: fn@crate::poll
///
/// # Examples
///
/// ```
/// use mio_st::event::{Event, Id, Ready};
///
/// let event = Event::new(Id(0), Ready::READABLE | Ready::WRITABLE);
///
/// assert_eq!(event.id(), Id(0));
/// assert_eq!(event.readiness(), Ready::READABLE | Ready::WRITABLE);
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Event {
    id: Id,
    readiness: Ready,
}

impl Event {
    /// Creates a new `Event` containing `id` and `readiness`.
    pub fn new(id: Id, readiness: Ready) -> Event {
        Event { id, readiness }
    }

    /// Returns the event's id.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns the event's readiness.
    pub fn readiness(&self) -> Ready {
        self.readiness
    }
}

/// Identifier of an event.
///
/// This is used to associate a readiness notifications with event handle.
///
/// See [`poll`] for more documentation on polling.
///
/// [`poll`]: fn@crate::poll
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
/// [`poll`]: fn@crate::poll
///
/// # Examples
///
/// ```
/// use mio_st::event::Ready;
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

const READABLE: u8 = 1 << 0;
const WRITABLE: u8 = 1 << 1;
const ERROR: u8 = 1 << 2;
const TIMER: u8 = 1 << 3;
#[cfg(unix)]
const HUP: u8 = 1 << 4;

impl Ready {
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

    /// Empty set of readiness.
    #[inline]
    pub(crate) fn empty() -> Ready {
        Ready(0)
    }

    /// Insert another readiness, same operation as `|=`.
    #[inline]
    pub(crate) fn insert(&mut self, other: Ready) {
        self.0 |= other.0;
    }

    /// Whether or not all flags in `other` are contained within `self`.
    #[inline]
    pub fn contains(self, other: Ready) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if the value includes readable readiness.
    #[inline]
    pub fn is_readable(self) -> bool {
        self.contains(Self::READABLE)
    }

    /// Returns true if the value includes writable readiness.
    #[inline]
    pub fn is_writable(self) -> bool {
        self.contains(Self::WRITABLE)
    }

    /// Returns true if the value includes error readiness.
    #[inline]
    pub fn is_error(self) -> bool {
        self.contains(Self::ERROR)
    }

    /// Returns true if a deadline has elapsed.
    #[inline]
    pub fn is_timer(self) -> bool {
        self.contains(Self::TIMER)
    }

    /// Returns true if the value includes HUP readiness.
    #[inline]
    #[cfg(unix)]
    pub fn is_hup(self) -> bool {
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

        if first {
            $f.write_str("(empty)")?;
        }

        Ok(())
    }}
}

impl fmt::Debug for Ready {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_debug!(self, f, READABLE, WRITABLE, ERROR, TIMER, HUP)
    }
}
