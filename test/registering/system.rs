use std::error::Error;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::net::TcpStream;
use mio_st::poll::PollOption;

use crate::{expect_events, init_with_poll};

#[test]
fn registering_deregistering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_reregistering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOption::Edge).unwrap();
    poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);
}

#[test]
fn registering_reregistering_deregistering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOption::Edge).unwrap();
    poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOption::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_deregistering_registering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOption::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();
    poll.register(&mut stream, EventedId(1), Ready::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::WRITABLE),
    ]);
}

#[test]
#[ignore = "currently possible, but shouldn't be"]
fn reregistering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    let result = poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot reregister"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
#[ignore = "currently possible, but shouldn't be"]
fn deregistering() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    let result = poll.deregister(&mut stream);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot deregister"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
#[ignore = "currently possible, but shouldn't be"]
fn registering_twice() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();
    let result = poll.register(&mut stream, EventedId(1), Ready::WRITABLE, PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot register"));

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);
}

#[test]
fn invalid_id() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    let invalid_id = EventedId(usize::max_value());

    let result = poll.register(&mut stream, invalid_id, Ready::READABLE, PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("invalid evented id"));
    expect_events(&mut poll, &mut events, 1, vec![]);

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();

    let result = poll.reregister(&mut stream, invalid_id, Ready::READABLE, PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("invalid evented id"));
    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn empty_interests() {
    let (mut poll, mut events) = init_with_poll();
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    let result = poll.register(&mut stream, EventedId(0), Ready::empty(), PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("empty interests"));
    expect_events(&mut poll, &mut events, 1, vec![]);

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();

    let result = poll.reregister(&mut stream, EventedId(0), Ready::empty(), PollOption::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("empty interests"));
    expect_events(&mut poll, &mut events, 1, vec![]);
}
