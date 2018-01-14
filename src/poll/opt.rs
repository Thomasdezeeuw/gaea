bitflags! {
    /// Options supplied when registering an `Evented` handle with `Poll`.
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
    #[inline]
    pub fn is_edge(&self) -> bool {
        self.contains(PollOpt::EDGE)
    }

    /// Returns true if the options include level-triggered notifications.
    #[inline]
    pub fn is_level(&self) -> bool {
        self.contains(PollOpt::LEVEL)
    }

    /// Returns true if the options includes oneshot.
    #[inline]
    pub fn is_oneshot(&self) -> bool {
        self.contains(PollOpt::ONESHOT)
    }
}
