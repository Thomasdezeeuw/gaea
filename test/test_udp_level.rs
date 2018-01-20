use std::thread;
use std::time::Duration;

use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::event::Event;
use mio::net::UdpSocket;
use expect_events;

#[test]
pub fn test_udp_level_triggered() {
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);

    // Create the listener
    let mut tx = UdpSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut rx = UdpSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();

    poll.register(&mut tx, Token(0), Ready::READABLE | Ready::WRITABLE, PollOpt::LEVEL).unwrap();
    poll.register(&mut rx, Token(1), Ready::READABLE | Ready::WRITABLE, PollOpt::LEVEL).unwrap();

    for _ in 0..2 {
        expect_events(&mut poll, &mut events, 2, vec![
            Event::new(Token(0), Ready::WRITABLE),
            Event::new(Token(1), Ready::WRITABLE),
        ]);
    }

    tx.send_to(b"hello world!", rx.local_addr().unwrap()).unwrap();

    thread::sleep(Duration::from_millis(200));

    for _ in 0..2 {
        expect_events(&mut poll, &mut events, 2, vec![
            Event::new(Token(1), Ready::READABLE | Ready::WRITABLE),
        ]);
    }

    let mut buf = [0; 200];
    while rx.recv_from(&mut buf).is_ok() {}

    for _ in 0..2 {
        expect_events(&mut poll, &mut events, 4, vec![
            Event::new(Token(1), Ready::WRITABLE),
        ]);
    }

    tx.send_to(b"hello world!", rx.local_addr().unwrap()).unwrap();
    thread::sleep(Duration::from_millis(200));

    expect_events(&mut poll, &mut events, 10, vec![
        Event::new(Token(1), Ready::READABLE | Ready::WRITABLE),
    ]);

    drop(rx);
}
