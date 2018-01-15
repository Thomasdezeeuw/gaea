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
//! use mio::*;
//! use mio::net::{TcpListener, TcpStream};
//!
//! // Setup some tokens to allow us to identify which event is
//! // for which socket.
//! const SERVER: Token = Token(0);
//! const CLIENT: Token = Token(1);
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
//!     for event in &events {
//!         match event.token() {
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
extern crate iovec;
extern crate lazycell;
#[macro_use]
extern crate log;
extern crate net2;

#[cfg(unix)]
extern crate libc;

#[cfg(target_os = "fuchsia")]
extern crate fuchsia_zircon as zircon;
#[cfg(target_os = "fuchsia")]
extern crate fuchsia_zircon_sys as zircon_sys;

#[cfg(windows)]
extern crate miow;

#[cfg(windows)]
extern crate winapi;

#[cfg(windows)]
extern crate kernel32;

mod poll2;
mod sys;
mod token;
mod ready;

pub use token::Token;
pub use ready::Ready;

// TODO: move event as submodule of poll, rexport `Events` and `Event` in poll.
pub mod event;
pub mod net;
pub mod poll;
pub mod registration;
pub mod timer;

// TODO: fix the imports in other files.
pub use poll::{Poll, PollOpt};
pub use registration::{Registration, SetReadiness};
pub use event::{Event, Events};

#[cfg(all(unix, not(target_os = "fuchsia")))]
pub mod unix {
    //! Unix only extensions.

    pub use sys::EventedFd;
}

#[cfg(target_os = "fuchsia")]
pub mod fuchsia {
    //! Fuchsia-only extensions.
    //!
    //! # Stability
    //!
    //! This module depends on the [magenta-sys crate](https://crates.io/crates/magenta-sys)
    //! and so might introduce breaking changes, even on minor releases,
    //! so long as that crate remains unstable.

    pub use sys::EventedHandle;
    pub use sys::fuchsia::{FuchsiaReady, zx_signals_t};
}

#[cfg(windows)]
pub mod windows {
    //! Windows-only extensions.
    //!
    //! Mio on windows is currently implemented with IOCP for a high-performance
    //! implementation of asynchronous I/O. Mio then provides TCP and UDP as
    //! sample bindings for the system to connect networking types to
    //! asynchronous I/O. On Unix this scheme is then also extensible to all
    //! other file descriptors with the `EventedFd` type, but on Windows no such
    //! analog is available. The purpose of this module, however, is to
    //! similarly provide a mechanism for foreign I/O types to get hooked up
    //! into the IOCP event loop.
    //!
    //! This module provides two types for interfacing with a custom IOCP
    //! handle:
    //!
    //! * `Binding` - this type is intended to govern binding with mio's `Poll`
    //!   type. Each I/O object should contain an instance of `Binding` that's
    //!   interfaced with for the implementation of the `Evented` trait. The
    //!   `register`, `reregister`, and `deregister` methods for the `Evented`
    //!   trait all have rough analogs with `Binding`.
    //!
    //!   Note that this type **does not handle readiness**. That is, this type
    //!   does not handle whether sockets are readable/writable/etc. It's
    //!   intended that IOCP types will internally manage this state with a
    //!   `SetReadiness` type from the `poll` module. The `SetReadiness` is
    //!   typically lazily created on the first time that `Evented::register` is
    //!   called and then stored in the I/O object.
    //!
    //!   Also note that for types which represent streams of bytes the mio
    //!   interface of *readiness* doesn't map directly to the Windows model of
    //!   *completion*. This means that types will have to perform internal
    //!   buffering to ensure that a readiness interface can be provided. For a
    //!   sample implementation see the TCP/UDP modules in mio itself.
    //!
    //! * `Overlapped` - this type is intended to be used as the concrete
    //!   instances of the `OVERLAPPED` type that most win32 methods expect.
    //!   It's crucial, for safety, that all asynchronous operations are
    //!   initiated with an instance of `Overlapped` and not another
    //!   instantiation of `OVERLAPPED`.
    //!
    //!   Mio's `Overlapped` type is created with a function pointer that
    //!   receives a `OVERLAPPED_ENTRY` type when called. This
    //!   `OVERLAPPED_ENTRY` type is defined in the `winapi` crate. Whenever a
    //!   completion is posted to an IOCP object the `OVERLAPPED` that was
    //!   signaled will be interpreted as `Overlapped` in the mio crate and this
    //!   function pointer will be invoked. Through this function pointer, and
    //!   through the `OVERLAPPED` pointer, implementations can handle
    //!   management of I/O events.
    //!
    //! When put together these two types enable custom Windows handles to be
    //! registered with mio's event loops. The `Binding` type is used to
    //! associate handles and the `Overlapped` type is used to execute I/O
    //! operations. When the I/O operations are completed a custom function
    //! pointer is called which typically modifies a `SetReadiness` set by
    //! `Evented` methods which will get later hooked into the mio event loop.

    pub use sys::{Overlapped, Binding};
}
