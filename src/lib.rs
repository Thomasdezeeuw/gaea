//! A fast, low-level IO library for Rust focusing on non-blocking APIs, event
//! notification for building high performance I/O apps.
//!
//! # Goals
//!
//! * Fast - minimal overhead over the equivalent OS facilities (epoll, kqueue, etc.).
//! * Zero allocations at runtime.
//! * A scalable readiness-based API.
//! * Provide utilities such as a timers and user space event queues.
//!
//! # Usage
//!
//! Using mio starts by creating a [`Poller`], which used to poll events, both
//! from the OS (backed by epoll, kqueue, etc.) and from user space.
//!
//! For more detail, including supported platforms, see [`Poller`].
//!
//! [`Poller`]: poll/struct.Poller.html
//!
//! # Undefined behaviour
//!
//! It is undefined how `Poller` will behave after a process is forked, if you
//! need fork a process do it before creating a `Poller` instance.
//!
//! As this is the single threaded version of mio, no types implement [`Sync`]
//! or [`Send`] and sharing these types across threads will result in undefined
//! behaviour.
//!
//! [`Sync`]: https://doc.rust-lang.org/nightly/std/marker/trait.Sync.html
//! [`Send`]: https://doc.rust-lang.org/nightly/std/marker/trait.Send.html
//!
//! # Examples
//!
//! A simple TCP server.
//!
//! ```
//! # fn main() -> Result<(), Box<std::error::Error>> {
//! use std::io;
//! use std::collections::HashMap;
//!
//! use mio_st::event::EventedId;
//! use mio_st::net::{TcpListener, TcpStream};
//! use mio_st::poll::{Poller, PollOption};
//!
//! // An unique id to associate an event with a handle, in this case for our
//! // TCP listener.
//! const SERVER_ID: EventedId = EventedId(0);
//!
//! // Create a `Poller` instance.
//! let mut poller = Poller::new()?;
//! // Also create a container for all events.
//! let mut events = Vec::new();
//!
//! // Setup the server listener.
//! let address = "127.0.0.1:12345".parse()?;
//! let mut server = TcpListener::bind(address)?;
//!
//! // Register our TCP listener with `Poller`, this allows us to receive
//! // notifications about incoming connections.
//! poller.register(&mut server, SERVER_ID, TcpListener::INTERESTS, PollOption::Edge)?;
//!
//! // A hashmap with `EventedId` -> `TcpStream` connections.
//! let mut connections = HashMap::with_capacity(512);
//!
//! // A simple "counter" to create new unique ids for each incoming connection.
//! let mut current_id = EventedId(10);
//!
//! // Start the event loop.
//! # let i = 0;
//! loop {
//! #   if i == 0 { break; }
//!     // Check for new events.
//!     poller.poll(&mut events, None)?;
//!
//!     for event in &mut events {
//!         // Depending on the event id we need to take an action.
//!         match event.id() {
//!             SERVER_ID => {
//!                 // The server is ready to accept one or more connections.
//!                 accept_connections(&mut server, &mut poller, &mut connections, &mut current_id)?;
//!             }
//!             connection_id => {
//!                 // A connection is possibly ready, but it might a spurious
//!                 // event.
//!                 let connection = match connections.get_mut(&connection_id) {
//!                     Some(connection) => connection,
//!                     // Spurious event, we can safely ignore it.
//!                     None => continue,
//!                 };
//!
//!                 // Do something with the connection.
//!                 # drop(connection)
//!             },
//!         }
//!     }
//! }
//!
//! fn accept_connections(server: &mut TcpListener, poller: &mut Poller, connections: &mut HashMap<EventedId, TcpStream>, current_id: &mut EventedId) -> io::Result<()> {
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
//!         *current_id = EventedId(current_id.0 + 1);
//!
//!         println!("got a new connection from: {}, id: {:?}", address, id);
//!
//!         // Register the TCP connection so we can handle events for it as
//!         // well.
//!         poller.register(&mut connection, id, TcpStream::INTERESTS, PollOption::Edge)?;
//!
//!         // Store our connection so we can access it later.
//!         connections.insert(id, connection);
//!     }
//! }
//!
//! fn would_block(err: &io::Error) -> bool {
//!     err.kind() == io::ErrorKind::WouldBlock
//! }
//! #     Ok(())
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
use std::io;
use std::time::Duration;

use log::trace;

mod sys;

pub mod event;
pub mod net;
pub mod os;
pub mod poll;
pub mod timers;
pub mod user_space;

#[cfg(unix)]
pub mod unix {
    //! Unix only extensions.

    pub use crate::sys::EventedFd;
    pub use crate::sys::EventedIo;
    pub use crate::sys::pipe::{new_pipe, Receiver, Sender};
}

#[doc(no_inline)]
pub use crate::event::{EventedId, Events, Ready};
#[doc(no_inline)]
pub use crate::poll::{BlockingPoll, Poll};

/// Poll a number of event sources for new events.
///
/// This will first poll `blocking_source` for readiness events, blocking at
/// most for a duration specified in `timeout`. Next it will poll all the other
/// `sources` for readiness events.
pub fn poll<BP, Evts>(blocking_source: &mut BP, sources: &mut [&mut dyn Poll<Evts>], events: &mut Evts, timeout: Option<Duration>) -> io::Result<()>
    where Evts: Events,
          BP: BlockingPoll<Evts>,
{
    trace!("polling: timeout={:?}", timeout);

    // Compute the maximum timeout we can use.
    let timeout = sources.iter().fold(timeout, |timeout, poller| {
        min_timeout(timeout, poller.next_event_available())
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
