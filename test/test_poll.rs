use std::time::Duration;

use mio::event::Events;
use mio::poll::{Poll, PollOpt, Ready, Token};
use mio::registration::Registration;

#[test]
fn test_poll_closes_fd() {
    for _ in 0..2000 {
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(4);
        let (mut registration, notifier) = Registration::new();

        poll.register(&mut registration, Token(0), Ready::READABLE, PollOpt::EDGE).unwrap();
        poll.poll(&mut events, Some(Duration::from_millis(0))).unwrap();

        drop(poll);
        drop(notifier);
        drop(registration);
    }
}

#[test]
#[cfg(all(unix, not(target_os = "fuchsia")))]
pub fn test_poll_as_raw_fd() {
    use std::os::unix::io::AsRawFd;
    let poll = Poll::new().unwrap();
    assert!(poll.as_raw_fd() > 0);
}
