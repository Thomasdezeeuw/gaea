//! Platform specific types.
//!
//! Each platform must have at least the following types:
//!
//! - `Selector`: system selector, e.g. `kqueue` or `epoll`, used by `OsQueue`.
//! - `TcpStream`: TCP stream, used in the net module.
//! - `TcpListener`: TCP listener, used in the net module.
//! - `UdpSocket`: UDP socket, used in the net module.
//! - `Awakener`: cross-thread awakener, used by `Awakener`.
//! - `Signals`: process signal handler, used in `Signals`.

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use self::unix::*;

/// Size of sack allocated system events array.
const EVENTS_CAP: usize = 128;
