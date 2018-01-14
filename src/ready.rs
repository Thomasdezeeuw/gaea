bitflags! {
    /// A set of readiness event kinds.
    ///
    /// `Ready` is a set of operation descriptors indicating which kind of an
    /// operation is ready to be performed. For example, `Ready::READABLE`
    /// indicates that the associated `Evented` handle is ready to perform a
    /// `read` operation.
    ///
    /// `Ready` values can be combined together using the various bitwise
    /// operators.
    ///
    /// For high level documentation on polling and readiness, see [`Poll`].
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::Ready;
    ///
    /// let ready = Ready::READABLE | Ready::WRITABLE;
    ///
    /// assert!(ready.is_readable());
    /// assert!(ready.is_writable());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    pub struct Ready: u8 {
        /// Readable readiness
        const READABLE = 0b0000001;
        /// Writable readiness.
        const WRITABLE = 0b0000010;
        /// Error readiness.
        const ERROR    = 0b0000100;
        /// Hup readiness, this signal is Unix specific.
        #[cfg(all(unix, not(target_os = "fuchsia")))]
        const HUP      = 0b0010000;
        #[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos"))]
        /// AIO completion readiness, this signal is specific to the BSD family.
        const AIO      = 0b0100000;
        #[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
        /// LIO completion readiness, this signal is specific to DragonFly and
        /// FreeBSD.
        const LIO      = 0b1000000;
    }
}

impl Ready {
    /// Returns true if the value includes readable readiness.
    #[inline]
    pub fn is_readable(&self) -> bool {
        self.contains(Ready::READABLE)
    }

    /// Returns true if the value includes writable readiness.
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.contains(Ready::WRITABLE)
    }

    /// Returns true if the value includes error readiness.
    #[inline]
    pub fn is_error(&self) -> bool {
        self.contains(Ready::ERROR)
    }

    /// Returns true if the value includes HUP readiness.
    #[inline]
    #[cfg(all(unix, not(target_os = "fuchsia")))]
    pub fn is_hup(&self) -> bool {
        self.contains(Ready::HUP)
    }

    /// Returns true if the value includes AIO completion readiness.
    #[inline]
    #[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos"))]
    pub fn is_aio(&self) -> bool {
        self.contains(Ready::AIO)
    }

    /// Returns true if the value includes LIO completion readiness.
    #[inline]
    #[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
    pub fn is_lio(&self) -> bool {
        self.contains(Ready::LIO)
    }
}
