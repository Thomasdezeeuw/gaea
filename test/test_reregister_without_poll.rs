use std::thread;
use std::time::Duration;

use mio::*;
use mio::net::{TcpListener, TcpStream};

#[test]
pub fn test_reregister_different_without_poll() {
    let mut events = Events::with_capacity(1024);
    let mut poll = Poll::new().unwrap();

    // Create the listener
    let mut l = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();

    // Register the listener with `Poll`
    poll.register(&mut l, Token(0), Ready::READABLE, PollOpt::EDGE | PollOpt::ONESHOT).unwrap();

    let mut s1 = TcpStream::connect(l.local_addr().unwrap()).unwrap();
    poll.register(&mut s1, Token(2), Ready::READABLE, PollOpt::EDGE).unwrap();

    thread::sleep(Duration::from_millis(200));

    poll.reregister(&mut l, Token(0), Ready::WRITABLE, PollOpt::EDGE | PollOpt::ONESHOT).unwrap();

    poll.poll(&mut events, Some(Duration::from_millis(200))).unwrap();
    assert_eq!(events.into_iter().len(), 0);
}
