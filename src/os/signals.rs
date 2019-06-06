//! Module for handling signals.

use std::io;
use std::iter::FusedIterator;
use std::ops::BitOr;

use crate::event;
use crate::os::OsQueue;
use crate::sys;

/// Notifications of process signals.
///
///
/// # Notes
///
/// This will block all signals in the signal set given when creating `Signals`,
/// using `sigprocmask`. This means that the program is not interrupt, or in any
/// way notified of signal until the assiocated [`OsQueue`] is [polled].
///
/// [polled]: crate::poll
///
/// # Implementation notes
///
/// On platforms that support kqueue this will use the `EVFILT_SIGNAL` event
/// filter, see [implementation notes of the `os` module] to see what platform
/// supports kqueue. On Linux it uses [signalfd].
///
/// [implementation notes of the `os` module]: ../index.html#implementation-notes
/// [signalfd]: http://man7.org/linux/man-pages/man2/signalfd.2.html
///
/// # Examples
///
/// ```
/// use std::io;
///
/// use gaea::{event, OsQueue, poll};
/// use gaea::os::{Signal, Signals, SignalSet};
///
/// const SIGNAL_ID: event::Id = event::Id(10);
///
/// fn main() -> io::Result<()> {
///     let mut os_queue = OsQueue::new()?;
///     let mut events = Vec::new();
///
///     // Create a `Signals` instance that will catch signals for us.
///     let mut signals = Signals::new(&mut os_queue, SignalSet::all(), SIGNAL_ID)?;
///
///     # // Don't want to let the example run for ever.
///     # let awakener = gaea::os::Awakener::new(&mut os_queue, event::Id(20))?;
///     # awakener.wake()?;
///     #
///     loop {
///         poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;
///
///         for event in &mut events {
///             match event.id() {
///                 // Receive the signal send.
///                 SIGNAL_ID => match signals.receive()? {
///                     Some(Signal::Interrupt) => println!("Got interrupt signal"),
///                     Some(Signal::Terminate) => println!("Got terminate signal"),
///                     Some(Signal::Quit) => println!("Got quit signal"),
///                     _ => println!("Got unknown signal event: {:?}", event),
///                 },
/// #               event::Id(20) => return Ok(()),
///                 _ => println!("Got unexpected event: {:?}", event),
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Signals {
    inner: sys::Signals,
}

impl Signals {
    /// Create a new signal notifier.
    ///
    /// This will cause the associated `OsQueue` instance to receive events when
    /// the process receives one of the signals in the signal set.
    pub fn new(os_queue: &mut OsQueue, signals: SignalSet, id: event::Id) -> io::Result<Signals> {
        debug_assert!(signals.size() != 0, "can't create `Signals` with an empty signal set");
        sys::Signals::new(os_queue.selector(), signals, id)
            .map(|inner| Signals { inner })
    }

    /// Receive a signal, if any.
    pub fn receive(&mut self) -> io::Result<Option<Signal>> {
        self.inner.receive()
    }
}

/// Set of [`Signal`]s used in registering signal notifications with [`Signals`].
///
/// # Examples
///
/// ```
/// use gaea::os::{Signal, SignalSet};
///
/// // Signal set can be created by bit-oring (`|`) signals together.
/// let set: SignalSet = Signal::Interrupt | Signal::Quit;
/// assert_eq!(set.size(), 2);
///
/// assert!(set.contains(Signal::Interrupt));
/// assert!(set.contains(Signal::Quit));
/// assert!(!set.contains(Signal::Terminate));
/// assert!(set.contains(Signal::Interrupt | Signal::Quit));
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SignalSet(u8);

const INTERRUPT: u8 = 1;
const QUIT: u8 = 1 << 1;
const TERMINATE: u8 = 1 << 2;

impl SignalSet {
    /// Create an empty signal set.
    pub const fn empty() -> SignalSet {
        SignalSet(0)
    }

    /// Create a new set with all signals.
    pub const fn all() -> SignalSet {
        SignalSet(INTERRUPT | QUIT | TERMINATE)
    }

    /// Number of signals in the set.
    pub const fn size(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether or not all signals in `other` are contained within `self`.
    ///
    /// # Notes
    ///
    /// This can also be used with [`Signal`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gaea::os::{Signal, SignalSet};
    ///
    /// let set = SignalSet::all();
    ///
    /// assert!(set.contains(Signal::Interrupt));
    /// assert!(set.contains(Signal::Quit));
    /// assert!(set.contains(Signal::Interrupt | Signal::Quit));
    ///
    /// let empty = SignalSet::empty();
    /// assert!(!empty.contains(Signal::Terminate));
    /// assert!(!empty.contains(Signal::Terminate | Signal::Quit));
    /// ```
    pub fn contains<S>(self, other: S) -> bool
        where S: Into<SignalSet>,
    {
        let other = other.into();
        (self.0 & other.0) == other.0
    }
}

impl From<Signal> for SignalSet {
    fn from(signal: Signal) -> Self {
        SignalSet(match signal {
            Signal::Interrupt => INTERRUPT,
            Signal::Quit => QUIT,
            Signal::Terminate => TERMINATE,
        })
    }
}

impl BitOr for SignalSet {
    type Output = SignalSet;

    fn bitor(self, rhs: Self) -> Self {
        SignalSet(self.0 | rhs.0)
    }
}

impl BitOr<Signal> for SignalSet {
    type Output = SignalSet;

    fn bitor(self, rhs: Signal) -> Self {
        self | Into::<SignalSet>::into(rhs)
    }
}

impl IntoIterator for SignalSet {
    type Item = Signal;
    type IntoIter = SignalSetIter;

    fn into_iter(self) -> Self::IntoIter {
        SignalSetIter(self)
    }
}

/// Iterator implementation for [`SignalSet`].
///
/// # Notes
///
/// The order in which the signals are iterated over is undefined.
#[derive(Debug)]
pub struct SignalSetIter(SignalSet);

impl Iterator for SignalSetIter {
    type Item = Signal;

    fn next(&mut self) -> Option<Self::Item> {
        let n = (self.0).0.trailing_zeros();
        match n {
            0 => Some(Signal::Interrupt),
            1 => Some(Signal::Quit),
            2 => Some(Signal::Terminate),
            _ => None,
        }.map(|signal| {
            // Remove the signal from the set.
            (self.0).0 &= !(1 << n);
            signal
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.0.size();
        (size, Some(size))
    }

    fn count(self) -> usize {
        self.0.size()
    }
}

impl ExactSizeIterator for SignalSetIter {
    fn len(&self) -> usize {
        self.0.size()
    }
}

impl FusedIterator for SignalSetIter {}

/// Signal used in registering signal notifications with [`Signals`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Signal {
    /// Interrupt signal.
    ///
    /// This signal is received by the process when its controlling terminal
    /// wishes to interrupt the process. This signal will for example be send
    /// when Ctrl+C is pressed in most terminals.
    ///
    /// Corresponds to POSIX signal `SIGINT`.
    Interrupt,
    /// Termination request signal.
    ///
    /// This signal received when the process is requested to terminate. This
    /// allows the process to perform nice termination, releasing resources and
    /// saving state if appropriate. This signal will be send when using the
    /// `kill` command for example.
    ///
    /// Corresponds to POSIX signal `SIGTERM`.
    Terminate,
    /// Terminal quit signal.
    ///
    /// This signal is received when the process is requested to quit and
    /// perform a core dump.
    ///
    /// Corresponds to POSIX signal `SIGQUIT`.
    Quit,
}

impl Signal {
    /// Convert the signal into a raw Unix signal.
    pub(crate) fn into_raw(self) -> libc::c_int {
        match self {
            Signal::Interrupt => libc::SIGINT,
            Signal::Quit => libc::SIGQUIT,
            Signal::Terminate => libc::SIGTERM,
        }
    }

    /// Convert a raw Unix signal into a signal.
    pub(crate) fn from_raw(raw_signal: libc::c_int) -> Option<Signal> {
        match raw_signal {
            libc::SIGINT => Some(Signal::Interrupt),
            libc::SIGQUIT => Some(Signal::Quit),
            libc::SIGTERM => Some(Signal::Terminate),
            _ => None,
        }
    }
}

impl BitOr for Signal {
    type Output = SignalSet;

    fn bitor(self, rhs: Self) -> SignalSet {
        Into::<SignalSet>::into(self) | rhs
    }
}

impl BitOr<SignalSet> for Signal {
    type Output = SignalSet;

    fn bitor(self, rhs: SignalSet) -> SignalSet {
        rhs | self
    }
}

#[cfg(test)]
mod tests {
    use crate::os::Signal;

    // More tests can be found in `tests/signals.rs`. This is only tested here
    // because it's not part of the public API.

    #[test]
    fn signal_from_raw() {
        assert_eq!(Signal::from_raw(libc::SIGINT), Some(Signal::Interrupt));
        assert_eq!(Signal::from_raw(libc::SIGQUIT), Some(Signal::Quit));
        assert_eq!(Signal::from_raw(libc::SIGTERM), Some(Signal::Terminate));

        // Unsupported signals.
        assert_eq!(Signal::from_raw(libc::SIGSTOP), None);
    }

    #[test]
    fn signal_into_raw() {
        assert_eq!(Signal::Interrupt.into_raw(), libc::SIGINT);
        assert_eq!(Signal::Quit.into_raw(), libc::SIGQUIT);
        assert_eq!(Signal::Terminate.into_raw(), libc::SIGTERM);
    }

    #[test]
    fn raw_signal() {
        assert_eq!(Signal::from_raw(libc::SIGINT).unwrap().into_raw(), libc::SIGINT);
        assert_eq!(Signal::from_raw(libc::SIGQUIT).unwrap().into_raw(), libc::SIGQUIT);
        assert_eq!(Signal::from_raw(libc::SIGTERM).unwrap().into_raw(), libc::SIGTERM);
    }
}
