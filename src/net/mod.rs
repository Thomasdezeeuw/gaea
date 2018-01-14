//! Networking primitives
//!
//! The types provided in this module are non-blocking by default and are
//! designed to be portable across all supported Mio platforms. As long as the
//! [portability guidelines] are followed, the behavior should be identical no
//! matter the target platform.
//!
//! [portability guidelines]: ../struct.Poll.html#portability

use std::io;

use poll::Poll;

mod tcp;
mod udp;

pub use self::tcp::{TcpListener, TcpStream};
pub use self::udp::UdpSocket;

/// Used to associate an IO type with a Selector.
#[derive(Debug, Clone)]
struct SelectorId {
    id: usize,
}

impl SelectorId {
    fn new() -> SelectorId {
        SelectorId {
            id: 0,
        }
    }

    fn associate_selector(&mut self, poll: &Poll) -> io::Result<()> {
        let selector_id = self.id;
        let poll_id = poll.selector.id();

        if selector_id != 0 && selector_id != poll_id {
            Err(io::Error::new(io::ErrorKind::Other, "socket already registered"))
        } else {
            self.id = poll_id;
            Ok(())
        }
    }
}
