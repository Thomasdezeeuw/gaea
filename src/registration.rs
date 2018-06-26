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
//! use mio_st::event::{EventedId, Events, Ready};
//! use mio_st::poll::{Poll, PollOption};
//! use mio_st::registration::Registration;
//!
//! // Create our poll and events.
//! let mut poll = Poll::new()?;
//! let mut events = Events::new();
//!
//! // Create a new user space registration and register it with `poll`.
//! let (mut registration, mut notifier) = Registration::new();
//! // Note that `PollOption` doesn't matter here since this is entirely user space
//! // driven and not in our control.
//! poll.register(&mut registration, EventedId(0), Ready::READABLE | Ready::WRITABLE, PollOption::Edge)?;
//!
//! // Notify the `registration` of a new, readable readiness event.
//! notifier.notify(Ready::READABLE)?;
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
use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::{Rc, Weak};

use event::{Event, Evented, EventedId, Ready, INVALID_EVENTED_ID};
use poll::{Poll, PollCalled, PollOption};

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
/// # Notes
///
/// The `PollOption` provided to `register` and `reregister` are ignored, since
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
        let notifier = Notifier { inner: Rc::downgrade(&inner) };
        (Registration { inner }, notifier)
    }
}

impl Evented for Registration {
    fn register(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, _: PollOption, _: PollCalled) -> io::Result<()> {
        self.inner.register(poll, id, interests)
    }

    fn reregister(&mut self, poll: &mut Poll, id: EventedId, interests: Ready, _: PollOption, _: PollCalled) -> io::Result<()> {
        self.inner.reregister(poll, id, interests)
    }

    fn deregister(&mut self, _: &mut Poll, _: PollCalled) -> io::Result<()> {
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
/// use mio_st::event::{EventedId, Events, Ready};
/// use mio_st::poll::{Poll, PollOption};
/// use mio_st::registration::{NotifyError, Registration};
///
/// // Create our poll, events and registration.
/// let mut poll = Poll::new()?;
/// let mut events = Events::new();
/// let (mut registration, mut notifier) = Registration::new();
///
/// // Next we'll try to notify our registration, but it's not registered yet so
/// // it will return an error.
/// assert_eq!(notifier.notify(Ready::WRITABLE), Err(NotifyError::NotRegistered));
///
/// // So we'll register our registration. Take not of the readiness arguments,
/// // they'll come back later.
/// poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOption::Edge)?;
///
/// // Now we'll try to call notify again. But again an error is returned, this
/// // time it indicate the accompanying `Registration` has no interest in the
/// // `WRITABLE` readiness and so no event is created.
/// assert_eq!(notifier.notify(Ready::WRITABLE), Err(NotifyError::NoInterest));
///
/// // Trying to call notify with empty readiness will also result in an error.
/// assert_eq!(notifier.notify(Ready::empty()), Err(NotifyError::EmptyReadiness));
///
/// // We can get the registration's interests by calling `intersects`. This
/// // will only fail if the registration is dropped.
/// let readiness = notifier.interests()?;
///
/// // Now, with all that we know, we can finally notify our registration with
/// // the correct information.
/// notifier.notify(readiness)?;
///
/// // But now we're no longer interested in our registration, so we drop it.
/// drop(registration);
///
/// // Trying to call notify now will result in an error indicating the
/// // registration was dropped.
/// assert_eq!(notifier.notify(Ready::READABLE), Err(NotifyError::RegistrationGone));
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
    /// [empty]: ../event/struct.Ready.html#method.empty
    /// [error variants]: enum.NotifyError.html
    pub fn notify(&mut self, ready: Ready) -> Result<(), NotifyError> {
        self.inner.upgrade()
            .ok_or(NotifyError::RegistrationGone)
            .and_then(|inner| inner.notify(ready))
    }

    /// Returns the interests of the accompanying `Registration`. This will be
    /// [empty] if the `Registration` hasn't been registered yet, or has been
    /// deregistered, but not dropped.
    ///
    /// [empty]: ../event/struct.Ready.html#method.empty
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
    /// The poll instance to which the `Registration` is registered is gone.
    PollGone,
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
            NotifyError::PollGone => "poll instance registered to is gone"
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
struct RegistrationInner {
    id: Cell<EventedId>,
    interests: Cell<Ready>,
    // A weak reference to the user space events of a `Poll` instance. This will
    // be set once register is called.
    userspace_events_ref: Cell<Option<Weak<RefCell<Vec<Event>>>>>,
}

impl RegistrationInner {
    fn new() -> RegistrationInner {
        RegistrationInner {
            id: Cell::new(INVALID_EVENTED_ID),
            interests: Cell::new(Ready::empty()),
            userspace_events_ref: Cell::new(None),
        }
    }

    fn register(&self, poll: &mut Poll, id: EventedId, interests: Ready) -> io::Result<()> {
        if self.id.get().is_valid() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot register \
                               `Registration` twice without deregistering first, \
                               or use reregister"))
        } else {
            self.id.set(id);
            self.interests.set(interests);
            self.userspace_events_ref.set(Some(poll.get_userspace_events()));
            Ok(())
        }
    }

    fn reregister(&self, poll: &mut Poll, id: EventedId, interests: Ready) -> io::Result<()> {
        // The registration must be registered first, before it can be
        // reregistered. However it is allowed to deregister it before
        // reregistering. To allow for these combinations deregister only resets
        // the `id`, not the `interests`, which will be empty before
        // registering, for which we check here.
        if !self.id.get().is_valid() && self.interests.get().is_empty() {
            Err(io::Error::new(io::ErrorKind::Other, "cannot reregister \
                               `Registration` before registering first"))
        } else {
            self.id.set(id);
            self.interests.set(interests);
            self.userspace_events_ref.set(Some(poll.get_userspace_events()));
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

    fn notify(&self, ready: Ready) -> Result<(), NotifyError> {
        let interests = self.interests();
        let id = self.id.get();
        if !id.is_valid() {
            Err(NotifyError::NotRegistered)
        } else if ready.is_empty() {
            Err(NotifyError::EmptyReadiness)
        } else if !interests.intersects(ready) {
            Err(NotifyError::NoInterest)
        } else {
            match self.userspace_events_ref() {
                Some(userspace_events) => match userspace_events.upgrade() {
                    Some(userspace_events) => {
                        let event = Event::new(id, ready & interests);
                        trace!("adding user space event: id={}, readiness={:?}",
                            event.id(), event.readiness());
                        userspace_events.borrow_mut().push(event);
                        Ok(())
                    },
                    None => Err(NotifyError::NotRegistered),
                },
                None => Err(NotifyError::NotRegistered),
            }
        }
    }

    fn userspace_events_ref(&self) -> &Option<Weak<RefCell<Vec<Event>>>> {
        unsafe { &*self.userspace_events_ref.as_ptr() }
    }

    fn interests(&self) -> Ready {
        self.interests.get()
    }
}

impl fmt::Debug for RegistrationInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RegistrationInner")
            .field("id", &self.id)
            .field("interests", &self.interests)
            .field("userspace_events_ref", self.userspace_events_ref() )
            .finish()
    }
}
