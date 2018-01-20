//! User space registrations.
//!
//! `Registration` allows implementing [`Evented`] for types that cannot work
//! with the [system selector]. A `Registration` is always paired with a
//! `Notifier`, which is a handle to notify the registration of events.
//!
//! A `Registration` / `Notifier` pair is created by calling
//! [`Registration::new`]. At this point, the registration is not being
//! monitored by a [`Poll`] instance, so calls to [`notify`] will not result in
//! any events.
//!
//! When [`notify`] is called and the `Registration` is associated with a
//! [`Poll`] instance, a readiness event will be created and returned by
//! [`Poll.poll`].
//!
//! `Registration` implements [`Evented`], so it can be used with [`Poll`] using
//! the same [`register`], [`reregister`], and [`deregister`] functions used
//! with TCP, UDP, etc... types. Once registered with [`Poll`], readiness state
//! changes result in readiness events being dispatched to the [`Poll`] instance
//! with which `Registration` is registered.
//!
//! [`Evented`]: ../event/trait.Evented.html
//! [system selector]: ../poll/struct.Poll.html#implementation-notes
//! [`Registration::new`]: struct.Registration.html#method.new
//! [`Poll`]: ../poll/struct.Poll.html
//! [`Poll.poll`]: ../poll/struct.Poll.html#method.poll
//! [`notify`]: struct.Notifier.html#method.notify
//! [`register`]: ../poll/struct.Poll.html#method.register
//! [`reregister`]: ../poll/struct.Poll.html#method.reregister
//! [`deregister`]: ../poll/struct.Poll.html#method.deregister
//!
//! # Examples
//!
//! ```
//! # use std::error::Error;
//! # fn try_main() -> Result<(), Box<Error>> {
//! use mio::event::Events;
//! use mio::poll::{Ready, Poll, PollOpt, Token};
//! use mio::registration::Registration;
//!
//! // Create our poll and events.
//! let mut poll = Poll::new()?;
//! let mut events = Events::with_capacity(128);
//!
//! // Create a new user space registration and register it with `poll`.
//! let (mut registration, mut notifier) = Registration::new();
//! poll.register(&mut registration, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
//!
//! // Notify the `registration` of a new, readable readiness event.
//! assert!(notifier.notify(&mut poll, Ready::READABLE)?);
//!
//! poll.poll(&mut events, None);
//!
//! for event in &mut events {
//!     if event.token() == Token(0) && event.readiness().is_readable() {
//!         return Ok(());
//!     }
//! }
//! # unreachable!("should have received an event");
//! #     Ok(())
//! # }
//! #
//! # fn main() {
//! #     try_main().unwrap();
//! # }
//! ```

use std::{fmt, io};
use std::cell::Cell;
use std::error::Error;
use std::rc::{Rc, Weak};

use event::{Event, Evented};
use poll::{Poll, PollOpt, Ready, Token, INVALID_TOKEN};

/// Handle to a user space registration.
///
///
/// This is a handle to an user space registration and can be used to implement
/// [`Evented`] for types that cannot work with the [system selector]. A
/// `Registration` is always paired with a [`Notifier`], which allows notifying
/// the registration of events.
///
/// See the [module documentation] for more information.
///
/// # Note
///
/// The `PollOpt` provided to `register` and `reregister` are ignored, since
/// use space controls the notifing of readiness.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [system selector]: ../poll/struct.Poll.html#implementation-notes
/// [`Notifier`]: struct.Notifier.html
/// [module documentation]: index.html
#[derive(Debug)]
pub struct Registration {
    inner: Rc<RegistrationInner>,
}

impl Registration {
    /// Create a new user space registration and accompanying notify handle.
    pub fn new() -> (Registration, Notifier) {
        let inner = Rc::new(RegistrationInner::new());
        let set_readiness = Notifier { inner: Rc::downgrade(&inner) };
        (Registration { inner }, set_readiness)
    }
}

impl Evented for Registration {
    fn register(&mut self, _poll: &mut Poll, token: Token, interest: Ready, _: PollOpt) -> io::Result<()> {
        self.inner.register(token, interest);
        Ok(())
    }

    fn reregister(&mut self, _poll: &mut Poll, token: Token, interest: Ready, _: PollOpt) -> io::Result<()> {
        self.inner.reregister(token, interest);
        Ok(())
    }

    fn deregister(&mut self, _: &mut Poll) -> io::Result<()> {
        self.inner.deregister();
        Ok(())
    }
}

/// Handle to notify an accompanying `Registration` of events.
///
/// This handle can be used to notify the accompanying `Registration` of events.
/// It can be created by calling [`Registration::new`].
///
/// This be cloned to share the ability to notify the same `Registration`.
///
/// See the [module documentation] for more information.
///
/// [`Registration::new`]: struct.Registration.html#method.new
/// [module documentation]: index.html
#[derive(Clone, Debug)]
pub struct Notifier {
    inner: Weak<RegistrationInner>,
}

impl Notifier {
    /// Creates a new user space event with the provided `ready` and the `token`
    /// set when registering the accompanying `Registration`.
    ///
    /// This will return `false` if no event is created, this can happen in the
    /// following cases:
    ///
    /// * the accompanying `Registration` has not been registered,
    /// * the `Registration` has no interests (the registered interest is
    ///   [empty]), or
    /// * the provided `ready` doesn't match the `Registration`'s interest,
    ///   e.g. registered with interest `Ready::READABLE` and then `notify` is
    ///   called with `Ready::WRITABLE`.
    ///
    /// Otherwise this will return `true` to indicate an event has been created.
    ///
    /// [empty]: ../poll/struct.Ready.html#method.empty
    pub fn notify(&mut self, poll: &mut Poll, ready: Ready) -> Result<bool, RegistrationGone> {
        self.inner.upgrade()
            .ok_or(RegistrationGone)
            .map(|inner| inner.notify(poll, ready))
    }

    /// Returns the interest of the accompanying `Registration`. This will be
    /// [empty] if the `Registration` hasn't been registered yet, or has been
    /// deregistered, but not dropped.
    ///
    /// [empty]: ../poll/struct.Ready.html#method.empty
    pub fn interest(&mut self) -> Result<Ready, RegistrationGone> {
        self.inner.upgrade()
            .ok_or(RegistrationGone)
            .map(|inner| inner.interest())
    }
}

/// Error returned by [`Notifier`].
///
/// It means the accompanying `Registration` is gone (as in it's been dropped).
///
/// [`Notifier`]: struct.Notifier.html
#[derive(Debug, Eq, PartialEq)]
pub struct RegistrationGone;

impl fmt::Display for RegistrationGone {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for RegistrationGone {
    fn description(&self) -> &str {
        "accompanying registration is gone"
    }
}

/// The inside of a `Registration`, owned by it, and weakly shared with
/// (possibly multiple) `Notifier`s.
#[derive(Debug)]
struct RegistrationInner {
    token: Cell<Token>,
    interest: Cell<Ready>,
}

impl RegistrationInner {
    fn new() -> RegistrationInner {
        RegistrationInner{
            token: Cell::new(INVALID_TOKEN),
            interest: Cell::new(Ready::empty()),
        }
    }

    fn register(&self, token: Token, interest: Ready) {
        assert_eq!(self.token.get(), INVALID_TOKEN, "cannot reregistering \
                   `Registration` without deregistering first");
        self.reregister(token, interest);
    }

    fn reregister(&self, token: Token, interest: Ready) {
        self.token.set(token);
        self.interest.set(interest);
    }

    fn deregister(&self) {
        self.token.set(INVALID_TOKEN);
        self.interest.set(Ready::empty());
    }

    fn notify(&self, poll: &mut Poll, ready: Ready) -> bool {
        let interest = self.interest();
        // Only pass the ready bits we're interested in.
        let ready = ready & interest;
        if self.token.get() == INVALID_TOKEN || interest.is_empty() || ready.is_empty() {
            false
        } else {
            poll.userspace_add_event(Event::new(self.token.get(), ready));
            true
        }
    }

    fn interest(&self) -> Ready {
        self.interest.get()
    }
}
