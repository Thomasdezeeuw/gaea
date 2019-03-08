//! Platform specific types.
//!
//! Each platform must have at least the following types:
//!
//! - `Selector`: system selector, e.g. `kqueue` or `epoll`, used by `OsQueue`.
//! - `TcpStream`: TCP stream, used in the net module.
//! - `TcpListener`: TCP listener, used in the net module.
//! - `UdpSocket`: UDP socket, used in the net module.
//! - `Awakener`: cross-thread awakener, used by `Awakener`.

/// A macro to create an array of [`MaybeUninit`]
///
/// This macro constructs and uninitialized array of the type `[MaybeUninit<T>; N]`.
///
/// Taken from Rust std lib (rust/src/libcore/macros.rs).
macro_rules! uninitialized_array {
    ($t:ty; $size:expr) => (unsafe {
        MaybeUninit::<[MaybeUninit<$t>; $size]>::uninitialized().into_initialized()
    });
}

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use self::unix::*;

/// Size of sack allocated system events array.
const EVENTS_CAP: usize = 128;
