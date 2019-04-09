mod awakener;
mod eventedfd;
mod signals;
mod tcp;
mod udp;

pub mod pipe;

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

pub use self::awakener::Awakener;
pub use self::eventedfd::EventedFd;
pub use self::signals::Signals;
pub use self::tcp::{TcpListener, TcpStream};
pub use self::udp::UdpSocket;
