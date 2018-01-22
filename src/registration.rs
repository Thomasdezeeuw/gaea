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
//! use mio::event::{EventedId, Events};
//! use mio::poll::{Ready, Poll, PollOpt};
//! use mio::registration::Registration;
//!
//! // Create our poll and events.
//! let mut poll = Poll::new()?;
//! let mut events = Events::with_capacity(128);
//!
//! // Create a new user space registration and register it with `poll`.
//! let (mut registration, mut notifier) = Registration::new();
//! poll.register(&mut registration, EventedId(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
//!
//! // Notify the `registration` of a new, readable readiness event.
//! assert!(notifier.notify(&mut poll, Ready::READABLE)?);
//!
//! poll.poll(&mut events, None);
//!
//! for event in &mut events {
//!     if event.token() == EventedId(0) && event.readiness().is_readable() {
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

use event::{Event, EventedId, Evented, INVALID_EVENTED_ID};
use poll::{Poll, PollOpt, Ready};

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
    fn register(&mut self, _poll: &mut Poll, id: EventedId, interest: Ready, _: PollOpt) -> io::Result<()> {
        self.inner.register(id, interest)
    }

    fn reregister(&mut self, _poll: &mut Poll, id: EventedId, interest: Ready, _: PollOpt) -> io::Result<()> {
        self.inner.reregister(id, interest)
    }

    fn deregister(&mut self, _: &mut Poll) -> io::Result<()> {
        self.inner.deregister()
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
    /// Creates a new user space event with the provided `ready` and `id`
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
    /// [error variants]: enum.NotifyError.html
    pub fn notify(&mut self, poll: &mut Poll, ready: Ready) -> Result<(), NotifyError> {
        self.inner.upgrade()
            .ok_or(NotifyError::RegistrationGone)
            .and_then(|inner| inner.notify(poll, ready))
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
/// See the error variants for the cause of the error.
///
/// [`Notifier`]: struct.Notifier.html
#[derive(Debug, Eq, PartialEq)]
pub enum NotifyError {
    /// The accompanying `Registration` has not been registered.
    NotRegistered,
    /// The provided `readiness` is empty.
    EmptyReadiness,
    /// The accompanying `Registration` has no interest in the provided
    /// readiness, e.g. registered with interest `Ready::READABLE` and then
    /// `notify` is called with `Ready::WRITABLE`.
    NoInterest,
    /// The accompanying `Registration` is gone (as in it's been dropped). Same
    /// error as [`RegistrationGone`].
    ///
    /// [`RegistrationGone`]: struct.RegistrationGone.html
    RegistrationGone,
}

impl fmt::Display for NotifyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for NotifyError {
    fn description(&self) -> &str {
        match *self {
            NotifyError::NotRegistered => "accompanying registration is not registered",
            NotifyError::EmptyReadiness => "readiness is empty",
            NotifyError::NoInterest => "accompanying registration has no interest in the event",
            NotifyError::RegistrationGone => RegistrationGone.description(),
        }
    }
}

/// Error returned by [`Notifier.interest`].
///
/// It means the accompanying `Registration` is gone (as in it's been dropped).
/// This is the same error as [`NotifyError::RegistrationGone`].
///
/// [`Notifier.interest`]: struct.Notifier.html#method.interest
/// [`NotifyError::RegistrationGone`]: enum.NotifyError.html#variant.RegistrationGone
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
    id: Cell<EventedId>,
    interest: Cell<Ready>,
}

impl RegistrationInner {
    fn new() -> RegistrationInner {
        RegistrationInner{
            id: Cell::new(INVALID_EVENTED_ID),
            interest: Cell::new(Ready::empty()),
        }
    }

    fn register(&self, id: EventedId, interest: Ready) -> io::Result<()> {
        if self.id.get().is_valid() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot register \
                               `Registration` twice without deregistering first, \
                               or use reregister"))
        } else {
            self.reregister(id, interest)
        }
    }

    fn reregister(&self, id: EventedId, interest: Ready) -> io::Result<()> {
        self.id.set(id);
        self.interest.set(interest);
        Ok(())
    }

    fn deregister(&self) -> io::Result<()> {
        self.id.set(INVALID_EVENTED_ID);
        self.interest.set(Ready::empty());
        Ok(())
    }

    fn notify(&self, poll: &mut Poll, ready: Ready) -> Result<(), NotifyError> {
        let interest = self.interest();
        let id = self.id.get();
        if !id.is_valid() {
            Err(NotifyError::NotRegistered)
        } else if ready.is_empty() {
            Err(NotifyError::EmptyReadiness)
        } else if !interest.intersects(ready) {
            Err(NotifyError::NoInterest)
        } else {
            // Only pass the ready bits we're interested in.
            poll.userspace_add_event(Event::new(id, ready & interest));
            Ok(())
        }
    }

    fn interest(&self) -> Ready {
        self.interest.get()
    }
}
