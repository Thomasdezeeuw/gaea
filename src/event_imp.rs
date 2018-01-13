bitflags! {
    /// Options supplied when registering an `Evented` handle with `Poll`
    ///
    /// `PollOpt` values can be combined together using the various bitwise
    /// operators.
    ///
    /// For high level documentation on polling and poll options, see [`Poll`].
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::PollOpt;
    ///
    /// let opts = PollOpt::EDGE | PollOpt::ONESHOT;
    ///
    /// assert!(opts.is_edge());
    /// assert!(opts.is_oneshot());
    /// assert!(!opts.is_level());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    pub struct PollOpt: u8 {
        /// Edge-triggered notifications.
        const EDGE    = 0b0000001;
        /// Level-triggered notifications.
        const LEVEL   = 0b0000010;
        /// Oneshot notifications.
        const ONESHOT = 0b0000100;
    }
}

impl PollOpt {
    /// Returns true if the options include edge-triggered notifications.
    ///
    /// See [`Poll`] for more documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::PollOpt;
    ///
    /// let opt = PollOpt::EDGE;
    ///
    /// assert!(opt.is_edge());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    #[inline]
    pub fn is_edge(&self) -> bool {
        self.contains(PollOpt::EDGE)
    }

    /// Returns true if the options include level-triggered notifications.
    ///
    /// See [`Poll`] for more documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::PollOpt;
    ///
    /// let opt = PollOpt::LEVEL;
    ///
    /// assert!(opt.is_level());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    #[inline]
    pub fn is_level(&self) -> bool {
        self.contains(PollOpt::LEVEL)
    }

    /// Returns true if the options includes oneshot.
    ///
    /// See [`Poll`] for more documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::PollOpt;
    ///
    /// let opt = PollOpt::ONESHOT;
    ///
    /// assert!(opt.is_oneshot());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    #[inline]
    pub fn is_oneshot(&self) -> bool {
        self.contains(PollOpt::ONESHOT)
    }
}

bitflags! {
    /// A set of readiness event kinds
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
        #[cfg(any(target_os = "dragonfly",
            target_os = "freebsd", target_os = "ios", target_os = "macos"))]
        /// AIO completion readiness, this signal is specific to the BSD family.
        const AIO      = 0b0100000;
        #[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
        /// LIO completion readiness, this signal is specific to DragonFly and
        /// FreeBSD.
        const LIO      = 0b1000000;
    }
}

impl Ready {
    /// Returns true if the value includes readable readiness
    ///
    /// See [`Poll`] for more documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::Ready;
    ///
    /// let ready = Ready::READABLE;
    ///
    /// assert!(ready.is_readable());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    #[inline]
    pub fn is_readable(&self) -> bool {
        self.contains(Ready::READABLE)
    }

    /// Returns true if the value includes writable readiness
    ///
    /// See [`Poll`] for more documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::Ready;
    ///
    /// let ready = Ready::WRITABLE;
    ///
    /// assert!(ready.is_writable());
    /// ```
    ///
    /// [`Poll`]: struct.Poll.html
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.contains(Ready::WRITABLE)
    }
}
