use std::num::NonZeroU8;
use std::ops::BitOr;

/// Interests used in registering.
///
/// Interests are used in [registering][] [`Evented`] handles with [`Poller`],
/// they indicate what readiness should be monitored for. For example if a
/// socket is registered with [readable] interests and the socket becomes
/// writable, no event will be returned from [`poll`].
///
/// [registering]: struct.Poller.html#method.register
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poller`]: struct.Poller.html
/// [readable]: #associatedconstant.READABLE
/// [`poll`]: struct.Poller.html#method.poll
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Interests(NonZeroU8);

const READABLE: u8 = 1 << 0;
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
    pub fn is_readable(self) -> bool {
        (self.0.get() & READABLE) != 0
    }

    /// Returns true if the value includes writable interest.
    #[inline]
    pub fn is_writable(self) -> bool {
        (self.0.get() & WRITABLE) != 0
    }
}

impl BitOr for Interests {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Interests(unsafe { NonZeroU8::new_unchecked(self.0.get() | rhs.0.get()) })
    }
}
