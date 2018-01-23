mod eventedfd;
mod eventedio;
mod tcp;
mod udp;

#[cfg(target_os = "linux")]
mod epoll;

#[cfg(target_os = "linux")]
pub use self::epoll::{Events, Selector};

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
mod kqueue;

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
pub use self::kqueue::{Events, Selector};

pub use self::eventedfd::EventedFd;
pub use self::eventedio::EventedIo;
pub use self::tcp::{TcpStream, TcpListener};
pub use self::udp::UdpSocket;
