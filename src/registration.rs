//! User space registrations.
//!
//! `Registration` allows implementing [`Evented`] for types that cannot work
//! with the system selector. A `Registration` is always paired with a
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
//! with TCP, UDP, etc. types. Once registered with [`Poll`], readiness state
//! changes result in readiness events being dispatched to the [`Poll`] instance
//! with which `Registration` is registered.
//!
//! [`Evented`]: ../event/trait.Evented.html
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
//! use mio_st::event::{EventedId, Events};
//! use mio_st::poll::{Poll, PollOpt, Ready};
//! use mio_st::registration::Registration;
//!
//! // Create our poll and events.
//! let mut poll = Poll::new()?;
//! let mut events = Events::with_capacity(128, 128);
//!
//! // Create a new user space registration and register it with `poll`.
//! let (mut registration, mut notifier) = Registration::new();
//! // Note that `PollOpt` doesn't matter here since this is entirely user space
//! // driven and not in our control.
//! poll.register(&mut registration, EventedId(0), Ready::READABLE | Ready::WRITABLE, PollOpt::Edge)?;
//!
//! // Notify the `registration` of a new, readable readiness event.
//! notifier.notify(&mut poll, Ready::READABLE)?;
//!
//! poll.poll(&mut events, None);
//!
//! for event in &mut events {
//!     if event.id() == EventedId(0) && event.readiness().is_readable() {
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
use poll::{Poll, PollOpt, Ready, Private};

/// Handle to a user space registration.
///
///
/// This is a handle to an user space registration and can be used to implement
/// [`Evented`] for types that cannot work with the system selector. A
/// `Registration` is always paired with a [`Notifier`], which allows notifying
/// the registration of events.
///
/// See the [module documentation] for more information.
///
/// # Note
///
/// The `PollOpt` provided to `register` and `reregister` are ignored, since
/// user space controls the notifing of readiness.
///
/// [`Evented`]: ../event/trait.Evented.html
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
    fn register(&mut self, _poll: &mut Poll, id: EventedId, interests: Ready, _: PollOpt, _: Private) -> io::Result<()> {
        self.inner.register(id, interests)
    }

    fn reregister(&mut self, _poll: &mut Poll, id: EventedId, interests: Ready, _: PollOpt, _: Private) -> io::Result<()> {
        self.inner.reregister(id, interests)
    }

    fn deregister(&mut self, _: &mut Poll, _: Private) -> io::Result<()> {
        self.inner.deregister()
    }
}

/// Handle to notify an accompanying `Registration` of events.
///
/// This handle can be used to notify the accompanying `Registration` of events.
/// It can be created by calling [`Registration::new`].
///
/// This handle can be shared by simply cloning it. Both handles will notify the
/// same `Registration`.
///
/// See the [module documentation] for more information.
///
/// [`Registration::new`]: struct.Registration.html#method.new
/// [module documentation]: index.html
///
/// # Examples
///
/// The example below explains the possible errors returned by `notify`.
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio_st::event::{EventedId, Events};
/// use mio_st::poll::{Poll, PollOpt, Ready};
/// use mio_st::registration::{NotifyError, Registration};
///
/// // Create our poll, events and registration.
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(128, 128);
/// let (mut registration, mut notifier) = Registration::new();
///
/// // Next we'll try to notify our registration, but it's not registered yet so
/// // it will return an error.
/// assert_eq!(notifier.notify(&mut poll, Ready::WRITABLE), Err(NotifyError::NotRegistered));
///
/// // So we'll register our registration. Take not of the readiness arguments,
/// // they'll come back later.
/// poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge)?;
///
/// // Now we'll try to call notify again. But again an error is returned, this
/// // time it indicate the accompanying `Registration` has no interest in the
/// // `WRITABLE` readiness and so no event is created.
/// assert_eq!(notifier.notify(&mut poll, Ready::WRITABLE), Err(NotifyError::NoInterest));
///
/// // Trying to call notify with empty readiness will also result in an error.
/// assert_eq!(notifier.notify(&mut poll, Ready::empty()), Err(NotifyError::EmptyReadiness));
///
/// // We can get the registration's interests by calling `intersects`. This
/// // will only fail if the registration is dropped.
/// let readiness = notifier.interests()?;
///
/// // Now, with all that we know, we can finally notify our registration with
/// // the correct information.
/// notifier.notify(&mut poll, readiness)?;
///
/// // But now we're no longer interested in our registration, so we drop it.
/// drop(registration);
///
/// // Trying to call notify now will result in an error indicating the
/// // registration was dropped.
/// assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::RegistrationGone));
///
/// // Note however that only event that we did send will still be returned by
/// // polling.
/// poll.poll(&mut events, None);
/// for event in &mut events {
///     if event.id() == EventedId(0) && event.readiness().is_readable() {
///         return Ok(());
///     }
/// }
/// # unreachable!("should have received an event");
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Notifier {
    inner: Weak<RegistrationInner>,
}

impl Notifier {
    /// Creates a new user space event with the provided `ready` and `id`
    /// set when registering the accompanying `Registration`.
    ///
    /// This will return an error if the event can't be created, see the [error
    /// variants] for detailed possible error causes.
    ///
    /// [empty]: ../poll/struct.Ready.html#method.empty
    /// [error variants]: enum.NotifyError.html
    pub fn notify(&mut self, poll: &mut Poll, ready: Ready) -> Result<(), NotifyError> {
        self.inner.upgrade()
            .ok_or(NotifyError::RegistrationGone)
            .and_then(|inner| inner.notify(poll, ready))
    }

    /// Returns the interests of the accompanying `Registration`. This will be
    /// [empty] if the `Registration` hasn't been registered yet, or has been
    /// deregistered, but not dropped.
    ///
    /// [empty]: ../poll/struct.Ready.html#method.empty
    pub fn interests(&mut self) -> Result<Ready, RegistrationGone> {
        self.inner.upgrade()
            .ok_or(RegistrationGone)
            .map(|inner| inner.interests())
    }
}

/// Error returned by [`Notifier`].
///
/// See the error variants for the cause of the error.
///
/// [`Notifier`]: struct.Notifier.html
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
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

/// Error returned by [`Notifier.interests`].
///
/// It means the accompanying `Registration` is gone (as in it's been dropped).
/// This is the same error as [`NotifyError::RegistrationGone`].
///
/// [`Notifier.interests`]: struct.Notifier.html#method.interests
/// [`NotifyError::RegistrationGone`]: enum.NotifyError.html#variant.RegistrationGone
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
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
    interests: Cell<Ready>,
}

impl RegistrationInner {
    fn new() -> RegistrationInner {
        RegistrationInner{
            id: Cell::new(INVALID_EVENTED_ID),
            interests: Cell::new(Ready::empty()),
        }
    }

    fn register(&self, id: EventedId, interests: Ready) -> io::Result<()> {
        if !id.is_valid() {
            Err(EventedId::invalid_error())
        } else if self.id.get().is_valid() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot register \
                               `Registration` twice without deregistering first, \
                               or use reregister"))
        } else {
            self.id.set(id);
            self.interests.set(interests);
            Ok(())
        }
    }

    fn reregister(&self, id: EventedId, interests: Ready) -> io::Result<()> {
        // The registration must be registered first, before it can be
        // reregistered. However it is allowed to deregister it before
        // reregistering. To allow for these combinations deregister only resets
        // the `id`, not the `interests`, which will be empty before
        // registering, for which we check here.
        if !id.is_valid() {
            Err(EventedId::invalid_error())
        } else if !self.id.get().is_valid() && self.interests.get().is_empty() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot reregister \
                               `Registration` before registering first"))
        } else {
            self.id.set(id);
            self.interests.set(interests);
            Ok(())
        }
    }

    fn deregister(&self) -> io::Result<()> {
        if !self.id.get().is_valid() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot deregister \
                               `Registration` before registering first"))
        } else {
            self.id.set(INVALID_EVENTED_ID);
            // Leave interests as is, see `reregister`.
            Ok(())
        }
    }

    fn notify(&self, poll: &mut Poll, ready: Ready) -> Result<(), NotifyError> {
        let interests = self.interests();
        let id = self.id.get();
        if !id.is_valid() {
            Err(NotifyError::NotRegistered)
        } else if ready.is_empty() {
            Err(NotifyError::EmptyReadiness)
        } else if !interests.intersects(ready) {
            Err(NotifyError::NoInterest)
        } else {
            // Only pass the ready bits we're interested in.
            poll.userspace_add_event(Event::new(id, ready & interests));
            Ok(())
        }
    }

    fn interests(&self) -> Ready {
        self.interests.get()
    }
}
