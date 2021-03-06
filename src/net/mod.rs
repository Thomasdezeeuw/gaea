//! Networking primitives.
//!
//! The types provided in this module are non-blocking by default and are
//! designed to be portable across all supported platforms. As long as the
//! [portability guidelines] are followed, the behavior should be identical no
//! matter the target platform.
//!
//! [portability guidelines]: ../os/index.html#portability

mod tcp;
mod udp;

pub use self::tcp::{TcpListener, TcpStream};
pub use self::udp::UdpSocket;
