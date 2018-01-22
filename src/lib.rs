//! A fast, low-level IO library for Rust focusing on non-blocking APIs, event
//! notification for building high performance I/O apps.
//!
//! # Goals
//!
//! * Fast - minimal overhead over the equivalent OS facilities (epoll, kqueue, etc.).
//! * Zero allocations.
//! * A scalable readiness-based API.
//! * Provide utilities such as a timers.
//!
//! # Usage
//!
//! Using mio starts by creating a [`Poll`], which used to poll events, both
//! from the OS (backed by epoll, kqueue, etc.) and from user space.
//!
//! For more detail, see [`Poll`].
//!
//! [`Poll`]: poll/struct.Poll.html
//!
//! # Example
//!
//! ```
//! # use std::error::Error;
//! # fn try_main() -> Result<(), Box<Error>> {
//! use mio::event::{Events, EventedId};
//! use mio::net::{TcpListener, TcpStream};
//! use mio::poll::{Poll, PollOpt, Ready};
//!
//! // Unique ids to associate an event with a handle, in this can a TCP
//! // listener (server) or stream (client).
//! const SERVER: EventedId = EventedId(0);
//! const CLIENT: EventedId = EventedId(1);
//!
//! // Setup the server socket.
//! let addr = "127.0.0.1:12345".parse()?;
//! let mut server = TcpListener::bind(addr)?;
//!
//! // Create a `Poll` instance along with an events container.
//! let mut poll = Poll::new()?;
//! let mut events = Events::with_capacity(512, 512);
//!
//! // Register our TCP listener with `Poll`, this allows us to receive
//! // notifications about incoming connections.
//! poll.register(&mut server, SERVER, Ready::READABLE, PollOpt::Level)?;
//!
//! // Setup the client socket, connection to our server.
//! let mut sock = TcpStream::connect(addr)?;
//!
//! // Register the socket with `Poll`.
//! poll.register(&mut sock, CLIENT, Ready::READABLE, PollOpt::Edge)?;
//!
//! // Start the event loop.
//! loop {
//!     // Check for new events.
//!     poll.poll(&mut events, None)?;
//!
//!     for event in &mut events {
//!         // Depending on the event id we need to take an action.
//!         match event.id() {
//!             SERVER => {
//!                 // Accept and drop the socket immediately, this will close
//!                 // the socket and notify the client of the EOF.
//!                 let _ = server.accept();
//!             }
//!             CLIENT => {
//!                 // The server just shuts down the socket, let's just exit
//!                 // from our event loop.
//!                 return Ok(());
//!             }
//!             _ => unreachable!(),
//!         }
//!     }
//! }
//! #     Ok(())
//! # }
//! #
//! # fn main() {
//! #     try_main().unwrap();
//! # }
//! ```

#![warn(missing_debug_implementations,
        missing_docs,
        trivial_casts,
        trivial_numeric_casts,
        unused_import_braces,
        unused_qualifications,
        unused_results,
)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate net2;

#[cfg(unix)]
extern crate libc;

mod sys;

pub mod event;
pub mod net;
pub mod poll;
pub mod registration;

#[cfg(unix)]
pub mod unix {
    //! Unix only extensions.

    pub use sys::EventedFd;
}
