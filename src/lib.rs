//! A fast, low-level IO library for Rust focusing on non-blocking APIs, event
//! notification, and other useful utilities for building high performance IO
//! apps.
//!
//! # Goals
//!
//! * Fast - minimal overhead over the equivalent OS facilities (epoll, kqueue, etc...)
//! * Zero allocations
//! * A scalable readiness-based API, similar to epoll on Linux
//! * Design to allow for stack allocated buffers when possible (avoid double buffering).
//! * Provide utilities such as a timers, a notification channel, buffer abstractions, and a slab.
//!
//! # Usage
//!
//! Using mio starts by creating a [`Poll`], which reads events from the OS and
//! put them into [`Events`]. You can handle IO events from the OS with it.
//!
//! For more detail, see [`Poll`].
//!
//! [`Poll`]: struct.Poll.html
//! [`Events`]: struct.Events.html
//!
//! # Example
//!
//! ```
//! use mio::event::{Events, Evented, EventedId};
//! use mio::net::{TcpListener, TcpStream};
//! use mio::poll::{Poll, PollOpt, Ready};
//!
//! // Setup some ids to allow us to identify which event is for which socket.
//! const SERVER: EventedId = EventedId(0);
//! const CLIENT: EventedId = EventedId(1);
//!
//! let addr = "127.0.0.1:13265".parse().unwrap();
//!
//! // Setup the server socket
//! let mut server = TcpListener::bind(addr).unwrap();
//!
//! // Create a poll instance
//! let mut poll = Poll::new().unwrap();
//!
//! // Start listening for incoming connections
//! poll.register(&mut server, SERVER, Ready::READABLE, PollOpt::EDGE).unwrap();
//!
//! // Setup the client socket
//! let mut sock = TcpStream::connect(addr).unwrap();
//!
//! // Register the socket
//! poll.register(&mut sock, CLIENT, Ready::READABLE, PollOpt::EDGE).unwrap();
//!
//! // Create storage for events
//! let mut events = Events::with_capacity(1024);
//!
//! loop {
//!     poll.poll(&mut events, None).unwrap();
//!
//!     for event in &mut events {
//!         match event.id() {
//!             SERVER => {
//!                 // Accept and drop the socket immediately, this will close
//!                 // the socket and notify the client of the EOF.
//!                 let _ = server.accept();
//!             }
//!             CLIENT => {
//!                 // The server just shuts down the socket, let's just exit
//!                 // from our event loop.
//!                 return;
//!             }
//!             _ => unreachable!(),
//!         }
//!     }
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/mio/0.6.12")]
#![crate_name = "mio"]

#![deny(warnings, missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate bitflags;
extern crate lazycell;
#[macro_use]
extern crate log;
extern crate net2;

#[cfg(unix)]
extern crate libc;

mod sys;

// TODO: move event as submodule of poll, rexport `Events` and `Event` in poll.
pub mod event;
pub mod net;
pub mod poll;
pub mod registration;

// TODO: fix the imports in other files.
pub use poll::{Poll, PollOpt, Ready, Token};
pub use registration::{Registration, Notifier};
pub use event::{Event, Events};

#[cfg(unix)]
pub mod unix {
    //! Unix only extensions.

    pub use sys::EventedFd;
}
