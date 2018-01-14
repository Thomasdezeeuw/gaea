use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::event::Event;
use mio::net::UdpSocket;
use {expect_events, sleep_ms};

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
            Event::new(Ready::WRITABLE, Token(0)),
            Event::new(Ready::WRITABLE, Token(1)),
        ]);
    }

    tx.send_to(b"hello world!", rx.local_addr().unwrap()).unwrap();

    sleep_ms(250);

    for _ in 0..2 {
        expect_events(&mut poll, &mut events, 2, vec![
            Event::new(Ready::READABLE | Ready::WRITABLE, Token(1))
        ]);
    }

    let mut buf = [0; 200];
    while rx.recv_from(&mut buf).is_ok() {}

    for _ in 0..2 {
        expect_events(&mut poll, &mut events, 4, vec![Event::new(Ready::WRITABLE, Token(1))]);
    }

    tx.send_to(b"hello world!", rx.local_addr().unwrap()).unwrap();
    sleep_ms(250);

    expect_events(&mut poll, &mut events, 10,
                  vec![Event::new(Ready::READABLE | Ready::WRITABLE, Token(1))]);

    drop(rx);
}
