use nix;

mod awakener;
mod eventedfd;
mod io;
mod tcp;
mod udp;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod epoll;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::epoll::{Events, Selector};

#[cfg(any(target_os = "bitrig", target_os = "dragonfly",
          target_os = "freebsd", target_os = "ios", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
mod kqueue;

#[cfg(any(target_os = "bitrig", target_os = "dragonfly",
          target_os = "freebsd", target_os = "ios", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
pub use self::kqueue::{Events, Selector};

pub use self::awakener::Awakener;
pub use self::eventedfd::EventedFd;
pub use self::io::{Io, set_nonblock};
pub use self::tcp::{TcpStream, TcpListener};
pub use self::udp::UdpSocket;

pub use iovec::IoVec;

/// Helper function to convert an `nix::Error` into an `io::Error`, use with
/// `Result`'s `map_err`, e.g. `nix_fn().map_err(nix_to_io_error)?`.
fn nix_to_io_error(err: nix::Error) -> ::std::io::Error {
    use ::std::io;
    match err {
        nix::Error::Sys(errno) => io::Error::from_raw_os_error(errno as i32),
        nix::Error::InvalidPath => io::Error::new(io::ErrorKind::Other, "invalid path"),
        nix::Error::InvalidUtf8 => io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8 string"),
        nix::Error::UnsupportedOperation => io::Error::new(io::ErrorKind::Other, "unsupported operation"),
    }
}

trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

impl IsMinusOne for i32 {
    fn is_minus_one(&self) -> bool { *self == -1 }
}
impl IsMinusOne for isize {
    fn is_minus_one(&self) -> bool { *self == -1 }
}

fn cvt<T: IsMinusOne>(t: T) -> ::std::io::Result<T> {
    use std::io;

    if t.is_minus_one() {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}
