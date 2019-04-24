//! A low-level library to build event driven applications. The core of the
//! library is [`poll`], which polls multiple [event sources] for readiness
//! events. Based on these readiness events the application will continue, e.g.
//! by running a [`Future`].
//!
//! A number of readiness event sources are provided:
//!
//!  * [`OsQueue`]: a readiness event queue backed by the OS (epoll or kqueue).
//!  * [`Queue`]: a single threaded, user space queue.
//!  * [`Timers`]: a single threaded, deadline based readiness queue.
//!
//! [event sources]: event::Source
//! [`Future`]: std::future::Future
//!
//! # Getting started
//!
//! Using the crate starts by creating one or more [`event::Source`]s.
//!
//! ```
//! # use mio_st::{OsQueue, Queue};
//! # fn main() -> std::io::Result<()> {
//! // `OsQueue` implements `event::Source` and is backed by epoll or kqueue.
//! let os_queue = OsQueue::new()?;
//! // `Queue` is a user space readiness event queue, which also implements
//! // `event::Source`.
//! let queue = Queue::new();
//! # drop((os_queue, queue));
//! # Ok(())
//! # }
//! ```
//!
//! As the name suggest `event::Source`s are the sources of readiness events,
//! these can be polled for readiness events (we'll get back to this later).
//! This crate provides three [`OsQueue`], [`Queue`] and [`Timers`]. But as
//! `event::Source` is a trait it can be implemented outside of this crate.
//!
//! Next an [`event::Sink`] is required, this used to store the readiness events
//! from the event sources.
//!
//! ```
//! // `Vec`tor implements `event::Sink`.
//! let events = Vec::new();
//! # drop::<Vec<mio_st::Event>>(events);
//! ```
//!
//! Just like `event::Source`, `event::Sink` is also a trait. When, for example,
//! building some kind of runtime `event::Source` can be directly implemented on
//! the scheduler type and instead of adding an [`Event`] to a collection it
//! will schedule a process/[`Future`]/task to run. For convenience `Vec`tors
//! also implement `event::Sink`.
//!
//! Both the `event::Source`s and `event::Sink` should only be created once and
//! reused in each call to [`poll`]. After we created both we can start
//! [polling] the `event::Source`s.
//!
//! [polling]: poll
//!
//! ```
//! # use std::io;
//! # use std::time::Duration;
//! # use mio_st::{poll, OsQueue, Queue};
//! # fn main() -> io::Result<()> {
//! # let mut os_queue = OsQueue::new()?;
//! # let mut queue = Queue::new();
//! # // Let poll return quickly.
//! # queue.add(mio_st::Event::new(mio_st::event::Id(0), mio_st::Ready::READABLE));
//! # let mut events = Vec::new();
//! // Poll both `os_queue` and `queue` for readiness events, with a maximum
//! // timeout of 1 seconds. Here we use an `io::Error` as error, see `poll`
//! // docs for more information on handling errors from different event
//! // sources.
//! poll::<_, io::Error>(&mut [&mut os_queue, &mut queue], &mut events,
//!     Some(Duration::from_secs(1)))?;
//! # Ok(())
//! # }
//! ```
//!
//! After the `event::Source`s are polled our `event::Sink` will be filled with
//! readiness events, if there are any. These can be used to continue
//! processing. Stick all the above in a loop and you've got yourself an event
//! loop, congratulations!
//!
//! ```
//! use std::io;
//! use std::time::Duration;
//!
//! use mio_st::{poll, OsQueue, Queue};
//!
//! # fn main() -> std::io::Result<()> {
//! // Create our `event::Source`s.
//! let mut os_queue = OsQueue::new()?;
//! let mut queue = Queue::new();
//! # // Let poll return quickly.
//! # queue.add(mio_st::Event::new(mio_st::event::Id(0), mio_st::Ready::READABLE));
//!
//! // And our `event::Sink`.
//! let mut events = Vec::new();
//!
//! // TODO: add events and such here...
//!
//! // Our event loop.
//! loop {
//!     // Poll for readiness events.
//!     poll::<_, io::Error>(&mut [&mut os_queue, &mut queue], &mut events,
//!         Some(Duration::from_secs(1)))?;
//!
//!     // And process each event.
//!     for event in events.drain(..) {
//!         println!("Got event: id={}, readiness={:?}", event.id(),
//!             event.readiness());
//!     }
//!     # return Ok(());
//! }
//! # }
//! ```
//!
//! # Examples
//!
//! More complete examples of how to use the crate can be found in the examples
//! directory of the source code ([on GitHub]).
//!
//! [on GitHub]: https://github.com/Thomasdezeeuw/mio-st/tree/master/examples

#![warn(anonymous_parameters,
        bare_trait_objects,
        missing_debug_implementations,
        missing_docs,
        trivial_casts,
        trivial_numeric_casts,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications,
        unused_results,
        variant_size_differences,
)]

// Disallow warnings when running tests.
#![cfg_attr(test, deny(warnings))]

// Disallow warnings in examples, we want to set a good example after all.
#![doc(test(attr(deny(warnings))))]

// Disallow clippy warnings.
#![deny(clippy::all)]

#![cfg_attr(not(feature = "std"), no_std)]

use core::cmp::min;
use core::time::Duration;

use log::trace;

#[cfg(feature = "std")]
mod sys;
#[cfg(feature = "std")]
mod timers;
#[cfg(feature = "std")]
mod user_space;

pub mod event;
#[cfg(feature = "std")]
pub mod net;
#[cfg(feature = "std")]
pub mod os;

#[cfg(all(feature = "std", unix))]
pub mod unix {
    //! Unix only extensions.

    #[doc(inline)]
    pub use crate::sys::pipe::{new_pipe, Receiver, Sender};
    #[doc(inline)]
    pub use crate::sys::EventedFd;
}

#[cfg(feature = "std")]
pub use crate::timers::Timers;
#[cfg(feature = "std")]
pub use crate::user_space::Queue;

#[doc(no_inline)]
pub use crate::event::{Event, Ready};
#[doc(no_inline)]
#[cfg(feature = "std")]
pub use crate::os::OsQueue;

/// Poll event sources for readiness events.
///
/// This first determines the maximum timeout to use based on the provided
/// `timeout` and the provided `event_sources`. For example if one of the
/// sources is [`Timers`] with a deadline of 1 second and a supplied `timeout`
/// of 10 seconds we don't want to block for the whole 10 seconds and overrun
/// the deadline by 9 seconds. Instead we'll use 1 seconds as timeout.
///
/// Next it will use the computed timeout in a [blocking poll] call of the first
/// of the provided `event_sources` for readiness events. This call will block
/// the current thread until a readiness event is ready or the timeout has
/// elapsed. After the blocking poll the other event sources will be [polled]
/// for readiness events, without blocking the thread further.
///
/// Readiness events will be added to the supplied `event_sink`. If not all
/// events fit into the event sink, they will be returned in the next call to
/// `poll`.
///
/// Providing a `timeout` of `None` means that `poll` will block until the
/// `blocking_source` is awoken by an external factor, what this means is
/// different for each event source.
///
/// [blocking poll]: event::Source::blocking_poll
/// [polled]: event::Source::poll
///
/// # Handling different error types
///
/// Each `event::Source` might have a different *concrete* error type, for
/// example [`OsQueue`] has a *concrete* error type of [`io::Error`]. However we
/// would still like to have a single error type returned from a call to poll.
/// To facilitate this each event source will convert there *concrete* error
/// into a user defined error (the generic parameter `E`), this way different
/// error types can be collected into a single type. In most cases this will be
/// `io::Error`, but this can also be custom enum type as see the second example
/// below.
///
/// Note that some event source don't return an error, for example both
/// [`Queue`] and [`Timers`] don't return an error as they are both implemented
/// in user space and will accept any type as error type (as they don't use it).
///
/// [`io::Error`]: std::io::Error
///
/// # Examples
///
/// Polling from an [`OsQueue`], [`Queue`] and [`Timers`].
///
/// ```
/// use std::io;
/// use std::time::Instant;
///
/// use mio_st::{event, OsQueue, Timers, Queue, Event, Ready, poll};
///
/// # fn main() -> io::Result<()> {
/// // Our event sources.
/// let mut os_queue = OsQueue::new()?;
/// let mut timers = Timers::new();
/// let mut queue = Queue::new();
///
/// // Our event sink.
/// let mut event_sink = Vec::new();
///
/// // Add an event to one of our event sources.
/// timers.add_deadline(event::Id(0), Instant::now());
///
/// // Poll all event sources without a timeout.
/// poll::<_, io::Error>(&mut [&mut os_queue, &mut timers, &mut queue], &mut event_sink, None)?;
/// // Even though we didn't provide a timeout `poll` will return without
/// // blocking because an event is ready.
/// assert_eq!(event_sink[0], Event::new(event::Id(0), Ready::TIMER));
///
/// # Ok(())
/// # }
/// ```
///
/// Using a custom enum error that collects errors from the different event
/// sources.
///
/// ```
/// # use std::time::Duration;
/// #
/// # use mio_st::event;
/// use mio_st::poll;
///
/// /// Our custom `event::Source`s.
/// // Note: implementations not shown for brevity. See `event::Source` for an
/// // example implementation.
/// struct EventSource1;
/// struct EventSource2;
/// #
/// # impl<ES, E> event::Source<ES, E> for EventSource1
/// #     where ES: event::Sink,
/// #           E: From<SourceError1>,
/// # {
/// #     fn next_event_available(&self) -> Option<Duration> {
/// #         None
/// #     }
/// #
/// #     fn poll(&mut self, _event_sink: &mut ES) -> Result<(), E> {
/// #         Ok(())
/// #     }
/// # }
/// #
/// # impl<ES, E> event::Source<ES, E> for EventSource2
/// #     where ES: event::Sink,
/// #           E: From<SourceError2>,
/// # {
/// #     fn next_event_available(&self) -> Option<Duration> {
/// #         None
/// #     }
/// #
/// #     fn poll(&mut self, _event_sink: &mut ES) -> Result<(), E> {
/// #         Ok(())
/// #     }
/// # }
///
/// /// `event::Source` error types.
/// struct SourceError1;
/// struct SourceError2;
///
/// /// Our custom error type.
/// #[derive(Debug)]
/// enum MyError {
///     Source1,
///     Source2,
/// }
///
/// // Implementing `From` allows the error to be converted into our custom
/// // error type.
/// // Note: this is also implemented for `SourceError2`, but not show for
/// // brevity.
/// impl From<SourceError1> for MyError {
///     fn from(_err: SourceError1) -> MyError {
///         MyError::Source1
///     }
/// }
/// #
/// # impl From<SourceError2> for MyError {
/// #     fn from(_err: SourceError2) -> MyError {
/// #         MyError::Source2
/// #     }
/// # }
///
/// # fn main() -> Result<(), MyError> {
/// // Our event sources.
/// let mut event_source1 = EventSource1; // With error type `SourceError1`.
/// let mut event_source2 = EventSource2; // With error type `SourceError2`.
/// // And event sink.
/// let mut event_sink = Vec::new();
///
/// // Poll both event sources converting any errors into our `MyError` type.
/// poll::<_, MyError>(&mut [&mut event_source1, &mut event_source2], &mut event_sink, None)?;
///
/// // Handle events, etc.
/// # Ok(())
/// # }
/// ```
pub fn poll<ES, E>(
    event_sources: &mut [&mut dyn event::Source<ES, E>],
    event_sink: &mut ES,
    timeout: Option<Duration>,
) -> Result<(), E>
    where ES: event::Sink,
{
    trace!("polling: timeout={:?}", timeout);

    // Compute the maximum timeout we can use.
    let timeout = event_sources.iter().fold(timeout, |timeout, event_source| {
        min_timeout(timeout, event_source.next_event_available())
    });

    let mut iter = event_sources.iter_mut();
    if let Some(event_source) = iter.next() {
        // Start with polling the blocking source.
        event_source.blocking_poll(event_sink, timeout)?;

        // Next poll all non-blocking sources.
        for event_source in iter {
            event_source.poll(event_sink)?;
        }
    }

    Ok(())
}

/// Returns the smallest timeout of the two timeouts provided.
fn min_timeout(left: Option<Duration>, right: Option<Duration>) -> Option<Duration> {
    match (left, right) {
        (Some(left), Some(right)) => Some(min(left, right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
