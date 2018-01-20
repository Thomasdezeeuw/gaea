//! Platform specific types.
//!
//! Each platform must have at least the following types:
//!
//! * `Events`: system specific events, used in `Selector.select`, wrapped by
//!   `events::Events`.
//! * `Selector`: system selector, e.g. `kqueue` or `epoll`, used by `Poll`.
//! * `TcpStream`: TCP stream, used in the net module.
//! * `TcpListener`: TCP listener, used in the net module.
//! * `UdpSocket`: UDP socket, used in the net module.

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use self::unix::*;
