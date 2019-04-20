//! A low-level library to build event driven applications. The core of the
//! library is [`poll`], which polls multiple [event sources] for readiness
//! events. Based on these readiness events the application will continue, e.g.
//! by polling a [`Future`].
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
//! # Usage
//!
//! Using the library starts by creating one or more [event sources]. Next an
//! [event sink] is required, this used to store the events from the event
//! sources. But as it's a trait this can also be scheduler of some kind to
//! directly schedule processes for which a readiness event is added.
//!
//! Next the event source can be [polled], using the events sink and a timeout.
//! This will poll all sources and block until a readiness event is available in
//! any of the sources or until the timeout expires. Next it's the applications
//! turn to process each event. Do this in a loop and you've got yourself an
//! event loop.
//!
//! [event source]: event::Source
//! [event sink]: event::Sink
//! [polled]: poll
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
