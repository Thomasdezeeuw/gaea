bitflags! {
    /// A set of readiness event kinds.
    ///
    /// `Ready` is a set of operation descriptors indicating which kind of
    /// operation is ready to be performed. For example, `Ready::READABLE`
    /// indicates that the associated `Evented` handle is ready to perform a
    /// read operation.
    ///
    /// `Ready` values can be combined together using the various bitwise
    /// operators, see examples below.
    ///
    /// In [`Poll`]'s [`register`] and [`reregister`] methods this used to
    /// describe in what kind of readiness events an `Evented` handle is
    /// interested in.
    ///
    /// For high level documentation on polling and readiness, see [`Poll`].
    ///
    /// [`Poll`]: ../poll/struct.Poll.html
    /// [`register`]: ../poll/struct.Poll.html#method.register
    /// [`reregister`]: ../poll/struct.Poll.html#method.reregister
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
    pub struct Ready: u8 {
        /// Readable readiness
        const READABLE = 0b0000_0001;
        /// Writable readiness.
        const WRITABLE = 0b0000_0010;
        /// Error readiness.
        const ERROR    = 0b0000_0100;
        /// Timer was triggered, see [`Timer`].
        ///
        /// [`Timer`]: ../timer/struct.Timer.html
        const TIMER    = 0b0000_1000;
        /// Hup readiness, this signal is Unix specific.
        #[cfg(unix)]
        const HUP      = 0b0001_0000;
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

    /// Returns true if the value includes an timer.
    #[inline]
    pub fn is_timer(&self) -> bool {
        self.contains(Ready::TIMER)
    }

    /// Returns true if the value includes HUP readiness.
    #[inline]
    #[cfg(unix)]
    pub fn is_hup(&self) -> bool {
        self.contains(Ready::HUP)
    }
}
