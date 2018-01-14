use std::{fmt, io, ptr, mem, ops};
use std::sync::Arc;
use std::sync::atomic::{Ordering};

use super::Token;
use super::event_imp::{Ready, PollOpt};
use event::Evented;

use poll::*;

/// Handle to a user space `Poll` registration.
///
/// `Registration` allows implementing [`Evented`] for types that cannot work
/// with the [system selector]. A `Registration` is always paired with a
/// `SetReadiness`, which allows updating the registration's readiness state.
/// When [`set_readiness`] is called and the `Registration` is associated with a
/// [`Poll`] instance, a readiness event will be created and eventually returned
/// by [`poll`].
///
/// A `Registration` / `SetReadiness` pair is created by calling
/// [`Registration::new2`]. At this point, the registration is not being
/// monitored by a [`Poll`] instance, so calls to `set_readiness` will not
/// result in any readiness notifications.
///
/// `Registration` implements [`Evented`], so it can be used with [`Poll`] using
/// the same [`register`], [`reregister`], and [`deregister`] functions used
/// with TCP, UDP, etc... types. Once registered with [`Poll`], readiness state
/// changes result in readiness events being dispatched to the [`Poll`] instance
/// with which `Registration` is registered.
///
/// **Note**, before using `Registration` be sure to read the
/// [`set_readiness`] documentation and the [portability] notes. The
/// guarantees offered by `Registration` may be weaker than expected.
///
/// For high level documentation, see [`Poll`].
///
/// # Examples
///
/// ```
/// use mio::{Ready, Registration, Poll, PollOpt, Token};
/// use mio::event::Evented;
///
/// use std::io;
/// use std::time::Instant;
/// use std::thread;
///
/// pub struct Deadline {
///     when: Instant,
///     registration: Registration,
/// }
///
/// impl Deadline {
///     pub fn new(when: Instant) -> Deadline {
///         let (registration, set_readiness) = Registration::new2();
///
///         thread::spawn(move || {
///             let now = Instant::now();
///
///             if now < when {
///                 thread::sleep(when - now);
///             }
///
///             set_readiness.set_readiness(Ready::READABLE);
///         });
///
///         Deadline {
///             when: when,
///             registration: registration,
///         }
///     }
///
///     pub fn is_elapsed(&self) -> bool {
///         Instant::now() >= self.when
///     }
/// }
///
/// impl Evented for Deadline {
///     fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         self.registration.register(poll, token, interest, opts)
///     }
///
///     fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         self.registration.reregister(poll, token, interest, opts)
///     }
///
///     fn deregister(&self, poll: &Poll) -> io::Result<()> {
///         self.registration.deregister(poll)
///     }
/// }
/// ```
///
/// [system selector]: struct.Poll.html#implementation-notes
/// [`Poll`]: struct.Poll.html
/// [`Registration::new2`]: struct.Registration.html#method.new2
/// [`Evented`]: event/trait.Evented.html
/// [`set_readiness`]: struct.SetReadiness.html#method.set_readiness
/// [`register`]: struct.Poll.html#method.register
/// [`reregister`]: struct.Poll.html#method.reregister
/// [`deregister`]: struct.Poll.html#method.deregister
/// [portability]: struct.Poll.html#portability
pub struct Registration {
    inner: RegistrationInner,
}

unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

impl Registration {
    /// Create and return a new `Registration` and the associated
    /// `SetReadiness`.
    ///
    /// See [struct] documentation for more detail and [`Poll`]
    /// for high level documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Ready, Registration, Poll, PollOpt, Token};
    /// use std::thread;
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// thread::spawn(move || {
    ///     use std::time::Duration;
    ///     thread::sleep(Duration::from_millis(500));
    ///
    ///     set_readiness.set_readiness(Ready::READABLE);
    /// });
    ///
    /// let mut poll = Poll::new()?;
    /// poll.register(&registration, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::EDGE)?;
    ///
    /// let mut events = Events::with_capacity(256);
    ///
    /// loop {
    ///     poll.poll(&mut events, None);
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) && event.readiness().is_readable() {
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    /// [struct]: #
    /// [`Poll`]: struct.Poll.html
    pub fn new2() -> (Registration, SetReadiness) {
        // Allocate the registration node. The new node will have `ref_count`
        // set to 2: one SetReadiness, one Registration.
        let node = Box::into_raw(Box::new(ReadinessNode::new(
                    ptr::null_mut(), Token(0), Ready::empty(), PollOpt::empty(), 2)));

        let registration = Registration {
            inner: RegistrationInner {
                node: node,
            },
        };

        let set_readiness = SetReadiness {
            inner: RegistrationInner {
                node: node,
            },
        };

        (registration, set_readiness)
    }

    // TODO: Get rid of this (windows depends on it for now)
    pub(crate) fn new_priv(poll: &Poll, token: Token, interest: Ready, opt: PollOpt)
        -> (Registration, SetReadiness)
    {
        // Clone handle to the readiness queue, this bumps the ref count
        let queue = poll.readiness_queue.inner.clone();

        // Convert to a *mut () pointer
        let queue: *mut () = unsafe { mem::transmute(queue) };

        // Allocate the registration node. The new node will have `ref_count`
        // set to 3: one SetReadiness, one Registration, and one Poll handle.
        let node = Box::into_raw(Box::new(ReadinessNode::new(
                    queue, token, interest, opt, 3)));

        let registration = Registration {
            inner: RegistrationInner {
                node: node,
            },
        };

        let set_readiness = SetReadiness {
            inner: RegistrationInner {
                node: node,
            },
        };

        (registration, set_readiness)
    }
}

impl Evented for Registration {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.inner.update(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.inner.update(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner.update(poll, Token(0), Ready::empty(), PollOpt::empty())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // `flag_as_dropped` toggles the `dropped` flag and notifies
        // `Poll::poll` to release its handle (which is just decrementing
        // the ref count).
        if self.inner.state.flag_as_dropped() {
            // Can't do anything if the queuing fails
            let _ = self.inner.enqueue_with_wakeup();
        }
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Registration")
            .finish()
    }
}

/// Updates the readiness state of the associated `Registration`.
///
/// See [`Registration`] for more documentation on using `SetReadiness` and
/// [`Poll`] for high level polling documentation.
///
/// [`Poll`]: struct.Poll.html
/// [`Registration`]: struct.Registration.html
#[derive(Clone)]
pub struct SetReadiness {
    inner: RegistrationInner,
}

unsafe impl Send for SetReadiness {}
unsafe impl Sync for SetReadiness {}

impl SetReadiness {
    /// Returns the registration's current readiness.
    ///
    /// # Note
    ///
    /// There is no guarantee that `readiness` establishes any sort of memory
    /// ordering. Any concurrent data access must be synchronized using another
    /// strategy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Registration, Ready};
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// assert!(set_readiness.readiness().is_empty());
    ///
    /// set_readiness.set_readiness(Ready::READABLE)?;
    /// assert!(set_readiness.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn readiness(&self) -> Ready {
        self.inner.readiness()
    }

    /// Set the registration's readiness
    ///
    /// If the associated `Registration` is registered with a [`Poll`] instance
    /// and has requested readiness events that include `ready`, then a future
    /// call to [`Poll::poll`] will receive a readiness event representing the
    /// readiness state change.
    ///
    /// # Note
    ///
    /// There is no guarantee that `readiness` establishes any sort of memory
    /// ordering. Any concurrent data access must be synchronized using another
    /// strategy.
    ///
    /// There is also no guarantee as to when the readiness event will be
    /// delivered to poll. A best attempt will be made to make the delivery in a
    /// "timely" fashion. For example, the following is **not** guaranteed to
    /// work:
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Registration, Ready, Poll, PollOpt, Token};
    ///
    /// let mut poll = Poll::new()?;
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// poll.register(&registration, Token(0), Ready::READABLE, PollOpt::EDGE)?;
    ///
    /// // Set the readiness, then immediately poll to try to get the readiness
    /// // event
    /// set_readiness.set_readiness(Ready::READABLE)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// poll.poll(&mut events, None)?;
    ///
    /// // There is NO guarantee that the following will work. It is possible
    /// // that the readiness event will be delivered at a later time.
    /// let mut iter = events.into_iter();
    /// let event = iter.next().unwrap();
    /// assert_eq!(event.token(), Token(0));
    /// assert!(event.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// # Examples
    ///
    /// A simple example, for a more elaborate example, see the [`Evented`]
    /// documentation.
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Registration, Ready};
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// assert!(set_readiness.readiness().is_empty());
    ///
    /// set_readiness.set_readiness(Ready::READABLE)?;
    /// assert!(set_readiness.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`Registration`]: struct.Registration.html
    /// [`Evented`]: event/trait.Evented.html#examples
    /// [`Poll`]: struct.Poll.html
    /// [`Poll::poll`]: struct.Poll.html#method.poll
    pub fn set_readiness(&self, ready: Ready) -> io::Result<()> {
        self.inner.set_readiness(ready)
    }
}

impl fmt::Debug for SetReadiness {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SetReadiness")
            .finish()
    }
}

struct RegistrationInner {
    // Unsafe pointer to the registration's node. The node is ref counted. This
    // cannot "simply" be tracked by an Arc because `Poll::poll` has an implicit
    // handle though it isn't stored anywhere. In other words, `Poll::poll`
    // needs to decrement the ref count before the node is freed.
    node: *mut ReadinessNode,
}

impl RegistrationInner {
    /// Get the registration's readiness.
    fn readiness(&self) -> Ready {
        self.state.load(Ordering::Relaxed).readiness()
    }

    /// Set the registration's readiness.
    ///
    /// This function can be called concurrently by an arbitrary number of
    /// SetReadiness handles.
    fn set_readiness(&self, ready: Ready) -> io::Result<()> {
        // Load the current atomic state.
        let mut state = self.state.load(Ordering::Acquire);
        let mut next;

        loop {
            next = state;

            if state.is_dropped() {
                // Node is dropped, no more notifications
                return Ok(());
            }

            // Update the readiness
            next.set_readiness(ready);

            // If the readiness is not blank, try to obtain permission to
            // push the node into the readiness queue.
            if !next.effective_readiness().is_empty() {
                next.set_queued();
            }

            let actual = self.state.compare_and_swap(state, next, Ordering::AcqRel);

            if state == actual {
                break;
            }

            state = actual;
        }

        if !state.is_queued() && next.is_queued() {
            // We toggled the queued flag, making us responsible for queuing the
            // node in the MPSC readiness queue.
            self.enqueue_with_wakeup()?;
        }

        Ok(())
    }

    /// Update the registration details associated with the node
    fn update(&self, poll: &Poll, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()> {
        // First, ensure poll instances match
        //
        // Load the queue pointer, `Relaxed` is sufficient here as only the
        // pointer is being operated on. The actual memory is guaranteed to be
        // visible the `poll: &Poll` ref passed as an argument to the function.
        let mut queue = self.readiness_queue.load(Ordering::Relaxed);
        let other: &*mut () = unsafe { mem::transmute(&poll.readiness_queue.inner) };
        let other = *other;

        debug_assert!(mem::size_of::<Arc<ReadinessQueueInner>>() == mem::size_of::<*mut ()>());

        if queue.is_null() {
            // Attempt to set the queue pointer. `Release` ordering synchronizes
            // with `Acquire` in `ensure_with_wakeup`.
            let actual = self.readiness_queue.compare_and_swap(
                queue, other, Ordering::Release);

            if actual.is_null() {
                // The CAS succeeded, this means that the node's ref count
                // should be incremented to reflect that the `poll` function
                // effectively owns the node as well.
                //
                // `Relaxed` ordering used for the same reason as in
                // RegistrationInner::clone
                self.ref_count.fetch_add(1, Ordering::Relaxed);

                // Note that the `queue` reference stored in our
                // `readiness_queue` field is intended to be a strong reference,
                // so now that we've successfully claimed the reference we bump
                // the refcount here.
                //
                // Down below in `release_node` when we deallocate this
                // `RegistrationInner` is where we'll transmute this back to an
                // arc and decrement the reference count.
                mem::forget(poll.readiness_queue.clone());
            } else {
                // The CAS failed, another thread set the queue pointer, so ensure
                // that the pointer and `other` match
                if actual != other {
                    return Err(io::Error::new(io::ErrorKind::Other, "registration handle associated with another `Poll` instance"));
                }
            }

            queue = other;
        } else if queue != other {
            return Err(io::Error::new(io::ErrorKind::Other, "registration handle associated with another `Poll` instance"));
        }

        unsafe {
            let actual = &poll.readiness_queue.inner as *const _ as *const usize;
            debug_assert_eq!(queue as usize, *actual);
        }

        // The `update_lock` atomic is used as a flag ensuring only a single
        // thread concurrently enters the `update` critical section. Any
        // concurrent calls to update are discarded. If coordinated updates are
        // required, the Mio user is responsible for handling that.
        //
        // Acquire / Release ordering is used on `update_lock` to ensure that
        // data access to the `token_*` variables are scoped to the critical
        // section.

        // Acquire the update lock.
        if self.update_lock.compare_and_swap(false, true, Ordering::Acquire) {
            // The lock is already held. Discard the update
            return Ok(());
        }

        // Relaxed ordering is acceptable here as the only memory that needs to
        // be visible as part of the update are the `token_*` variables, and
        // ordering has already been handled by the `update_lock` access.
        let mut state = self.state.load(Ordering::Relaxed);
        let mut next;

        // Read the current token, again this memory has been ordered by the
        // acquire on `update_lock`.
        let curr_token_pos = state.token_write_pos();
        let curr_token = unsafe { self::token(self, curr_token_pos) };

        let mut next_token_pos = curr_token_pos;

        // If the `update` call is changing the token, then compute the next
        // available token slot and write the token there.
        //
        // Note that this computation is happening *outside* of the
        // compare-and-swap loop. The update lock ensures that only a single
        // thread could be mutating the write_token_position, so the
        // `next_token_pos` will never need to be recomputed even if
        // `token_read_pos` concurrently changes. This is because
        // `token_read_pos` can ONLY concurrently change to the current value of
        // `token_write_pos`, so `next_token_pos` will always remain valid.
        if token != curr_token {
            next_token_pos = state.next_token_pos();

            // Update the token
            match next_token_pos {
                0 => unsafe { *self.token_0.get() = token },
                1 => unsafe { *self.token_1.get() = token },
                2 => unsafe { *self.token_2.get() = token },
                _ => unreachable!(),
            }
        }

        // Now enter the compare-and-swap loop
        loop {
            next = state;

            // The node is only dropped once all `Registration` handles are
            // dropped. Only `Registration` can call `update`.
            debug_assert!(!state.is_dropped());

            // Update the write token position, this will also release the token
            // to Poll::poll.
            next.set_token_write_pos(next_token_pos);

            // Update readiness and poll opts
            next.set_interest(interest);
            next.set_poll_opt(opt);

            // If there is effective readiness, the node will need to be queued
            // for processing. This exact behavior is still TBD, so we are
            // conservative for now and always fire.
            //
            // See https://github.com/carllerche/mio/issues/535.
            if !next.effective_readiness().is_empty() {
                next.set_queued();
            }

            // compare-and-swap the state values. Only `Release` is needed here.
            // The `Release` ensures that `Poll::poll` will see the token
            // update and the update function doesn't care about any other
            // memory visibility.
            let actual = self.state.compare_and_swap(state, next, Ordering::Release);

            if actual == state {
                break;
            }

            // CAS failed, but `curr_token_pos` should not have changed given
            // that we still hold the update lock.
            debug_assert_eq!(curr_token_pos, actual.token_write_pos());

            state = actual;
        }

        // Release the lock
        self.update_lock.store(false, Ordering::Release);

        if !state.is_queued() && next.is_queued() {
            // We are responsible for enqueing the node.
            enqueue_with_wakeup(queue, self)?;
        }

        Ok(())
    }
}

impl ops::Deref for RegistrationInner {
    type Target = ReadinessNode;

    fn deref(&self) -> &ReadinessNode {
        unsafe { &*self.node }
    }
}

impl Clone for RegistrationInner {
    fn clone(&self) -> RegistrationInner {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        // As explained in the [Boost documentation][1], Increasing the
        // reference counter can always be done with memory_order_relaxed: New
        // references to an object can only be formed from an existing
        // reference, and passing an existing reference from one thread to
        // another must already provide any required synchronization.
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        let old_size = self.ref_count.fetch_add(1, Ordering::Relaxed);

        // However we need to guard against massive refcounts in case someone
        // is `mem::forget`ing Arcs. If we don't do this the count can overflow
        // and users will use-after free. We racily saturate to `isize::MAX` on
        // the assumption that there aren't ~2 billion threads incrementing
        // the reference count at once. This branch will never be taken in
        // any realistic program.
        //
        // We abort because such a program is incredibly degenerate, and we
        // don't care to support it.
        if old_size & !MAX_REFCOUNT != 0 {
            // TODO: This should really abort the process
            panic!();
        }

        RegistrationInner {
            node: self.node.clone(),
        }
    }
}

impl Drop for RegistrationInner {
    fn drop(&mut self) {
        // Only handles releasing from `Registration` and `SetReadiness`
        // handles. Poll has to call this itself.
        release_node(self.node);
    }
}
