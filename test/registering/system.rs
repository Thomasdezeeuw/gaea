use std::error::Error;

use mio_st::event::{Event, EventedId};
use mio_st::net::TcpStream;
use mio_st::poll::{Ready, PollOpt};

use {expect_events, init_with_poll};


#[test]
fn registering_deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_reregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOpt::Edge).unwrap();
    poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOpt::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);
}

#[test]
fn registering_reregistering_deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOpt::Edge).unwrap();
    poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_deregistering_registering() {
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::WRITABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut stream).unwrap();
    poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOpt::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);
}

#[test]
#[ignore = "currently possible, but shouldn't be"]
fn reregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    let result = poll.reregister(&mut stream, EventedId(1), Ready::READABLE, PollOpt::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot reregister"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
#[ignore = "currently possible, but shouldn't be"]
fn deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
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
    let (mut poll, mut events) = init_with_poll(8);
    let address = "127.0.0.1:12345".parse().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poll.register(&mut stream, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    let result = poll.register(&mut stream, EventedId(1), Ready::WRITABLE, PollOpt::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot register"));

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);
}
