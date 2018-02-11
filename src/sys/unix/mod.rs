mod eventedfd;
mod eventedio;
mod tcp;
mod udp;

pub mod pipe;

/// Size of sack allocated events array, this is indepent from the user defined
/// size of `Events`.
const EVENTS_CAP: usize = 512;

#[cfg(target_os = "linux")]
mod epoll;

#[cfg(target_os = "linux")]
pub use self::epoll::Selector;

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
mod kqueue;

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
pub use self::kqueue::Selector;

pub use self::eventedfd::EventedFd;
pub use self::eventedio::EventedIo;
pub use self::tcp::{TcpListener, TcpStream};
pub use self::udp::UdpSocket;
