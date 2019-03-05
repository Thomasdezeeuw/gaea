use std::fmt;
use std::ops::BitOr;

/// Option supplied when [registering] an [`Evented`] handle with [`OsQueue`].
///
/// For high level documentation on registering see [`OsQueue`].
///
/// [registering]: crate::os::OsQueue::register
/// [`OsQueue`]: crate::os::OsQueue
///
/// # Difference
///
/// An [`Evented`] registration may request [edge-triggered], [level-triggered]
/// or [oneshot] events. `Evented` handles registered with oneshot polling
/// option will only receive a single event and then have to [reregister] too
/// receive more events.
///
/// The difference between the edge-triggered and level-trigger can be described
/// as follows. Supposed that the following scenario happens:
///
/// 1. A [`TcpStream`] is registered with `OsQueue`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`poll`] returns the id associated with the socket indicating
///    readable readiness.
/// 4. 1kb is read from the socket.
/// 5. Another call to [`poll`] is made.
///
/// If when the socket was registered with `OsQueue`, and *edge*-triggered
/// events were requested, then the call to [`poll`] done in step **5** will
/// (probably) block despite there being another 1kb still present in the socket
/// read buffer. The reason for this is that edge-triggered mode delivers events
/// only when changes occur on the monitored [`Evented`]. So, in step *5* the
/// caller might end up waiting for some data that is already present inside the
/// socket buffer.
///
/// With edge-triggered events, operations **must** be performed on the
/// `Evented` type until [`WouldBlock`] is returned. In other words, after
/// receiving an event indicating readiness for a certain operation, one should
/// assume that [`poll`] may never return another event for the same id and
/// readiness until the operation returns [`WouldBlock`].
///
/// By contrast, when *level*-triggered notifications was requested, each call
/// to [`poll`] will return an event for the socket as long as data remains in
/// the socket buffer. Though generally, level-triggered events should be
/// avoided if high performance is a concern.
///
/// Since even with edge-triggered events, multiple events can be generated upon
/// receipt of multiple chunks of data, the caller has the option to set the
/// oneshot flag. This tells `OsQueue` to disable the associated [`Evented`]
/// after the event is returned from [`poll`], note that *disabled* and
/// *deregistered* are not the same thing. Subsequent calls to [`poll`] will no
/// longer include events for [`Evented`] handles that are disabled even if the
/// readiness state changes. The handle can be re-enabled by calling
/// [`reregister`]. When handles are disabled, internal resources used to
/// monitor the handle are maintained until the handle is deregistered. This
/// makes re-registering the handle a fast operation.
///
/// For example, in the following scenario:
///
/// 1. A [`TcpStream`] is registered with `OsQueue`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`poll`] returns the id associated with the socket indicating
///    readable readiness.
/// 4. 2kb is read from the socket.
/// 5. Another call to read is issued and [`WouldBlock`] is returned
/// 6. The socket receives another 2kb of data.
/// 7. Another call to [`poll`] is made.
///
/// Assuming the socket was registered with `OsQueue` with the *oneshot* option,
/// then the call to [`poll`] in step 7 would block. This is because, oneshot
/// tells `OsQueue` to disable events for the socket after returning an event.
///
/// In order to receive the event for the data received in step 6, the socket
/// would need to be reregistered using [`reregister`].
///
/// [`Evented`]: crate::os::Evented
/// [edge-triggered]: crate::os::PollOption::EDGE
/// [level-triggered]: crate::os::PollOption::LEVEL
/// [oneshot]: crate::os::PollOption::ONESHOT
/// [reregister]: crate::os::OsQueue::reregister
/// [`TcpStream`]: crate::net::TcpStream
/// [`poll`]: crate::poll
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
/// [`reregister`]: crate::os::OsQueue::reregister
///
/// # Notes
///
/// It is not possible to combine edge and level triggers.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct PollOption(u8);

// Level trigger is 0.
const EDGE: u8    = 1 << 0;
const ONESHOT: u8 = 1 << 1;

impl PollOption {
    /// Level-triggered notifications.
    pub const LEVEL: PollOption = PollOption(0);

    /// Edge-triggered notifications.
    pub const EDGE: PollOption = PollOption(EDGE);

    /// Oneshot notifications.
    pub const ONESHOT: PollOption = PollOption(ONESHOT);

    /// Returns true if the value includes level trigger.
    #[inline]
    pub fn is_level(self) -> bool {
        !self.is_edge()
    }

    /// Returns true if the value includes edge trigger.
    #[inline]
    pub fn is_edge(self) -> bool {
        self.0 & EDGE != 0
    }

    /// Returns true if the value includes oneshot trigger.
    #[inline]
    pub fn is_oneshot(self) -> bool {
        self.0 & ONESHOT != 0
    }
}

impl BitOr for PollOption {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        PollOption(self.0 | rhs.0)
    }
}

impl fmt::Debug for PollOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(match (self.is_edge(), self.is_oneshot()) {
            (false, false) => "LEVEL",
            (true, false) => "EDGE",
            (false, true) => "LEVEL | ONESHOT",
            (true, true) => "EDGE | ONESHOT",
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::os::PollOption;

    #[test]
    fn is_tests() {
        assert!(PollOption::LEVEL.is_level());
        assert!(!PollOption::LEVEL.is_edge());
        assert!(!PollOption::LEVEL.is_oneshot());

        assert!(!PollOption::EDGE.is_level());
        assert!(PollOption::EDGE.is_edge());
        assert!(!PollOption::EDGE.is_oneshot());

        assert!(PollOption::ONESHOT.is_level());
        assert!(!PollOption::ONESHOT.is_edge());
        assert!(PollOption::ONESHOT.is_oneshot());
    }

    #[test]
    fn bit_or() {
        let opt = PollOption::LEVEL | PollOption::ONESHOT;
        assert!(opt.is_level());
        assert!(!opt.is_edge());
        assert!(opt.is_oneshot());

        let opt = PollOption::EDGE | PollOption::ONESHOT;
        assert!(!opt.is_level());
        assert!(opt.is_edge());
        assert!(opt.is_oneshot());
    }

    #[test]
    fn fmt_debug() {
        assert_eq!(format!("{:?}", PollOption::LEVEL), "LEVEL");
        assert_eq!(format!("{:?}", PollOption::EDGE), "EDGE");
        assert_eq!(format!("{:?}", PollOption::ONESHOT), "LEVEL | ONESHOT");
        assert_eq!(format!("{:?}", PollOption::LEVEL | PollOption::ONESHOT), "LEVEL | ONESHOT");
        assert_eq!(format!("{:?}", PollOption::EDGE | PollOption::ONESHOT), "EDGE | ONESHOT");
    }
}
