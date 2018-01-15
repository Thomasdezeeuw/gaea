bitflags! {
    /// Options supplied when registering an `Evented` handle with `Poll`.
    ///
    /// `PollOpt` values can be combined together using the various bitwise
    /// operators.
    ///
    /// For high level documentation on polling and poll options, see [`Poll`].
    ///
    /// [`Poll`]: struct.Poll.html
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::poll::PollOpt;
    ///
    /// let opt = PollOpt::EDGE | PollOpt::ONESHOT;
    ///
    /// assert!(opt.is_edge());
    /// assert!(opt.is_oneshot());
    /// assert!(!opt.is_level());
    /// ```
    pub struct PollOpt: u8 {
        /// Edge-triggered notifications.
        const EDGE    = 0b001;
        /// Level-triggered notifications.
        const LEVEL   = 0b010;
        /// Oneshot notifications.
        const ONESHOT = 0b100;
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

    /// Returns true if the options includes oneshot notifications.
    #[inline]
    pub fn is_oneshot(&self) -> bool {
        self.contains(PollOpt::ONESHOT)
    }
}
