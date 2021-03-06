use std::fmt;
use std::num::NonZeroU8;
use std::ops::BitOr;

/// Interests supplied when [registering] an [`Evented`] handle with [`OsQueue`].
///
/// Interests are used in [registering][] [`Evented`] handles with [`OsQueue`],
/// they indicate what readiness should be monitored for. For example if a
/// socket is registered with [readable] interests and the socket becomes
/// writable, no event will be returned from [`poll`].
///
/// [registering]: crate::os::OsQueue::register
/// [`Evented`]: crate::os::Evented
/// [`OsQueue`]: crate::os::OsQueue
/// [readable]: Interests::READABLE
/// [`poll`]: crate::poll
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Interests(NonZeroU8);

const READABLE: u8 = 1;
const WRITABLE: u8 = 1 << 1;

impl Interests {
    /// Readable interest.
    pub const READABLE: Interests = Interests(unsafe { NonZeroU8::new_unchecked(READABLE) });

    /// Writable interest.
    pub const WRITABLE: Interests = Interests(unsafe { NonZeroU8::new_unchecked(WRITABLE) });

    /// Both readable and writable interests, not public because `Interests`
    /// might be expanded in the future.
    pub(crate) const BOTH: Interests = Interests(unsafe { NonZeroU8::new_unchecked(READABLE | WRITABLE) });

    /// Returns true if the value includes readable interest.
    #[inline]
    pub const fn is_readable(self) -> bool {
        self.0.get() & READABLE != 0
    }

    /// Returns true if the value includes writable interest.
    #[inline]
    pub const fn is_writable(self) -> bool {
        self.0.get() & WRITABLE != 0
    }
}

impl BitOr for Interests {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Interests(unsafe { NonZeroU8::new_unchecked(self.0.get() | rhs.0.get()) })
    }
}

impl fmt::Debug for Interests {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(match (self.is_readable(), self.is_writable()) {
            (true, true) => "READABLE | WRITABLE",
            (true, false) => "READABLE",
            (false, true) => "WRITABLE",
            (false, false) => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::os::Interests;

    #[test]
    fn is_tests() {
        assert!(Interests::READABLE.is_readable());
        assert!(!Interests::READABLE.is_writable());
        assert!(!Interests::WRITABLE.is_readable());
        assert!(Interests::WRITABLE.is_writable());
        assert!(Interests::BOTH.is_readable());
        assert!(Interests::BOTH.is_writable());
    }

    #[test]
    fn bit_or() {
        let interests = Interests::READABLE | Interests::WRITABLE;
        assert!(interests.is_readable());
        assert!(interests.is_writable());
    }

    #[test]
    fn fmt_debug() {
        assert_eq!(format!("{:?}", Interests::READABLE), "READABLE");
        assert_eq!(format!("{:?}", Interests::WRITABLE), "WRITABLE");
        assert_eq!(format!("{:?}", Interests::BOTH), "READABLE | WRITABLE");
    }
}
