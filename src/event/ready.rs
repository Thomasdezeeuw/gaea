use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// A set of readiness event kinds.
///
/// `Ready` is a set of operation descriptors indicating which kind of operation
/// is ready to be performed. For example, `Ready::READABLE` indicates that the
/// associated `Evented` handle is ready to perform a read operation.
///
/// `Ready` values can be combined together using the various bitwise operators,
/// see examples below.
///
/// For high level documentation on polling and readiness, see [`Poller`].
///
/// [`Poller`]: ../poll/struct.Poller.html
/// [`register`]: ../poll/struct.Poller.html#method.register
/// [`reregister`]: ../poll/struct.Poller.html#method.reregister
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

    /// Deadline was elapsed, see [`Poller.add_deadline`].
    ///
    /// [`Poller.add_deadline`]: ../poll/struct.Poller.html#method.add_deadline
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

    /// Whether or not all flags in `other`are contained within `self`.
    #[inline]
    pub fn contains(self, other: Ready) -> bool {
        self.0 & other.0 != 0
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

#[cfg(test)]
mod tests {
    use crate::event::Ready;

    #[test]
    fn is_tests() {
        assert!(!Ready::empty().is_readable());
        assert!(!Ready::empty().is_writable());
        assert!(!Ready::empty().is_error());
        assert!(!Ready::empty().is_timer());
        #[cfg(unix)]
        assert!(!Ready::empty().is_hup());

        assert!(Ready::READABLE.is_readable());
        assert!(!Ready::READABLE.is_writable());
        assert!(!Ready::READABLE.is_error());
        assert!(!Ready::READABLE.is_timer());
        #[cfg(unix)]
        assert!(!Ready::READABLE.is_hup());

        assert!(!Ready::WRITABLE.is_readable());
        assert!(Ready::WRITABLE.is_writable());
        assert!(!Ready::WRITABLE.is_error());
        assert!(!Ready::WRITABLE.is_timer());
        #[cfg(unix)]
        assert!(!Ready::WRITABLE.is_hup());

        assert!(!Ready::ERROR.is_readable());
        assert!(!Ready::ERROR.is_writable());
        assert!(Ready::ERROR.is_error());
        assert!(!Ready::ERROR.is_timer());
        #[cfg(unix)]
        assert!(!Ready::ERROR.is_hup());

        assert!(!Ready::TIMER.is_readable());
        assert!(!Ready::TIMER.is_writable());
        assert!(!Ready::TIMER.is_error());
        assert!(Ready::TIMER.is_timer());
        #[cfg(unix)]
        assert!(!Ready::TIMER.is_hup());

        #[cfg(unix)]
        {
            assert!(!Ready::HUP.is_readable());
            assert!(!Ready::HUP.is_writable());
            assert!(!Ready::HUP.is_error());
            assert!(!Ready::HUP.is_timer());
            assert!(Ready::HUP.is_hup());
        }
    }

    #[test]
    fn bit_or() {
        let readiness = Ready::READABLE | Ready::WRITABLE | Ready::ERROR;
        assert!(readiness.is_readable());
        assert!(readiness.is_writable());
        assert!(readiness.is_error());
        assert!(!readiness.is_timer());
        #[cfg(unix)]
        assert!(!readiness.is_hup());
    }

    #[test]
    fn bit_or_assign() {
        let mut readiness = Ready::READABLE;
        readiness |= Ready::WRITABLE;
        assert!(readiness.is_readable());
        assert!(readiness.is_writable());
        assert!(!readiness.is_error());
        assert!(!readiness.is_timer());
        #[cfg(unix)]
        assert!(!readiness.is_hup());
    }

    #[test]
    fn fmt_debug() {
        assert_eq!(format!("{:?}", Ready::READABLE), "READABLE");
        assert_eq!(format!("{:?}", Ready::WRITABLE), "WRITABLE");
        assert_eq!(format!("{:?}", Ready::ERROR), "ERROR");
        assert_eq!(format!("{:?}", Ready::TIMER), "TIMER");
        #[cfg(unix)]
        assert_eq!(format!("{:?}", Ready::HUP), "HUP");

        assert_eq!(format!("{:?}", Ready::empty()), "(empty)");

        assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE), "READABLE | WRITABLE");
        assert_eq!(format!("{:?}", Ready::ERROR | Ready::TIMER), "ERROR | TIMER");
        assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE | Ready::ERROR | Ready::TIMER),
            "READABLE | WRITABLE | ERROR | TIMER");
    }
}
