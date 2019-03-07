//! A low-level library to build event driven applications. The core of the
//! library is [`poll`], which polls multiple [event sources] for readiness
//! events. Based on these readiness event the application will continue, e.g.
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
//! Using the library starts by creating a [blocking event source] and zero or
//! more (nonblocking) [event sources]. Next an [events container] is required,
//! this used to store the events from the event source, but as it's a trait
//! this can also be scheduler of some kind to directly schedule processes for
//! which a readiness event is generated.
//!
//! Next the event source can be [polled], using the events container and a
//! timeout. This will poll all sources and block until a readiness event is
//! available in any of the sources or until the timeout expires. Next it's the
//! applications turn to process each event. Do this in a loop and you've got
//! yourself an event loop.
//!
//! [blocking event source]: event::BlockingSource
//! [event source]: event::Source
//! [events container]: Events
//! [polled]: poll
//!
//! # Examples
//!
//! The example below shows a simple non-blocking TCP server.
//!
//! ```
//! # fn main() -> Result<(), Box<std::error::Error>> {
//! use std::io;
//! use std::collections::HashMap;
//!
//! use mio_st::net::{TcpListener, TcpStream};
//! use mio_st::os::{OsQueue, RegisterOption};
//! use mio_st::{event, poll};
//!
//! // An unique id to associate an event with a handle, in this case for our
//! // TCP listener.
//! const SERVER_ID: event::Id = event::Id(0);
//!
//! // Create a Operating System backed (epoll or kqueue) queue.
//! // This is a blocking event source as it implements `BlockingSource`.
//! let mut os_queue = OsQueue::new()?;
//! // Crate our events container.
//! let mut events = Vec::new();
//!
//! // Setup a TCP listener, which will act as our server.
//! let address = "127.0.0.1:12345".parse()?;
//! let mut server = TcpListener::bind(address)?;
//!
//! // Register our TCP listener with `OsQueue`, this allows us to receive
//! // readiness events about incoming connections.
//! os_queue.register(&mut server, SERVER_ID, TcpListener::INTERESTS, RegisterOption::EDGE)?;
//!
//! // A hashmap with `event::Id` -> `TcpStream` connections.
//! let mut connections = HashMap::new();
//!
//! // A simple "counter" to create new unique ids for each incoming connection.
//! let mut current_id = event::Id(10);
//!
//! // Start our event loop.
//! # let i = 0; // Don't run the event loop.
//! loop {
//! #   if i == 0 { return Ok(()) }
//!     // Poll for events. As we only have a single event source we provided an
//!     // empty array as second argument.
//!     poll::<_, _, io::Error>(&mut os_queue, &mut [], &mut events, None)?;
//!
//!     // Process each event.
//!     for event in events.drain(..) {
//!         // Depending on the event id we need to take an action.
//!         match event.id() {
//!             SERVER_ID => {
//!                 // The server is ready to accept one or more connections.
//!                 accept_connections(&mut server, &mut os_queue, &mut connections, &mut current_id)?;
//!             },
//!             connection_id => {
//!                 // A connection is possibly ready, but it might a spurious
//!                 // event.
//!                 let connection = match connections.get_mut(&connection_id) {
//!                     Some(connection) => connection,
//!                     // Spurious event, we can safely ignore it.
//!                     None => continue,
//!                 };
//!
//!                 // Do something with the connection...
//!                 # drop(connection)
//!             },
//!         }
//!     }
//! }
//!
//! fn accept_connections(server: &mut TcpListener, os_queue: &mut OsQueue, connections: &mut HashMap<event::Id, TcpStream>, current_id: &mut event::Id) -> io::Result<()> {
//!     // Since we registered with edge-triggered events for our server we need
//!     // to accept connections until we hit a would block "error".
//!     loop {
//!         let (mut connection, address) = match server.accept() {
//!             Ok((connection, address)) => (connection, address),
//!             Err(ref err) if would_block(err) => return Ok(()),
//!             Err(err) => return Err(err),
//!         };
//!
//!         // Generate a new id for the connection.
//!         let id = *current_id;
//!         *current_id = event::Id(current_id.0 + 1);
//!
//!         println!("got a new connection from: {}, id: {:?}", address, id);
//!
//!         // Register the TCP connection so we can handle events for it as
//!         // well.
//!         os_queue.register(&mut connection, id, TcpStream::INTERESTS, RegisterOption::EDGE)?;
//!
//!         // Store our connection so we can access it later.
//!         connections.insert(id, connection);
//!     }
//! }
//!
//! fn would_block(err: &io::Error) -> bool {
//!     err.kind() == io::ErrorKind::WouldBlock
//! }
//! # }
//! ```

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

use std::cmp::min;
use std::time::Duration;

use log::trace;

mod sys;
mod timers;
mod user_space;

pub mod event;
pub mod net;
pub mod os;

#[cfg(unix)]
pub mod unix {
    //! Unix only extensions.

    pub use crate::sys::EventedFd;
    pub use crate::sys::pipe::{new_pipe, Receiver, Sender};
}

pub use crate::timers::Timers;
pub use crate::user_space::Queue;

#[doc(no_inline)]
pub use crate::event::{Event, Events, Ready};
#[doc(no_inline)]
pub use crate::os::OsQueue;

/// Poll event sources for readiness events.
///
/// This first determines the maximum timeout to use based on the provided
/// `timeout` and the provided `sources`. For example if one of the sources is a
/// [`Timers`] with a deadline of 1 second and a supplied `timeout` of 10
/// seconds we don't want to block for the whole 10 seconds and over run the
/// deadline by 9 seconds. Instead internally we'll use 9 seconds as timeout.
///
/// Next it will use the timeout in a blocking poll call of `blocking_source`
/// for readiness events. This call will block the current thread until a
/// readiness event is ready or the timeout has elapsed. After the blocking poll
/// the other `sources` will be polled for readiness events, without blocking
/// the thread further.
///
/// Readiness events will be added to the supplied `events` container. If not
/// all events fit into the `events`, they will be returned on the next call to
/// `poll`.
///
/// Providing a `timeout` of `None` means that `poll` will block until the
/// `blocking_source` is awoken by an external factor, for example a readiness
/// event.
pub fn poll<BS, Evts, E>(
    blocking_source: &mut BS,
    sources: &mut [&mut dyn event::Source<Evts, E>],
    events: &mut Evts,
    timeout: Option<Duration>,
) -> Result<(), E>
    where BS: event::BlockingSource<Evts, E>,
          Evts: Events,
{
    trace!("polling: timeout={:?}", timeout);

    // Compute the maximum timeout we can use.
    let timeout = sources.iter().fold(timeout, |timeout, source| {
        min_timeout(timeout, source.next_event_available())
    });

    // Start with polling the blocking source.
    blocking_source.blocking_poll(events, timeout)?;

    // Next poll all non-blocking sources.
    for source in sources.iter_mut() {
        source.poll(events)?;
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
