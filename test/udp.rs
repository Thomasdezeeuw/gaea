use std::{io, thread};
use std::net::{self, SocketAddr};
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::net::{ConnectedUdpSocket, UdpSocket};
use mio_st::poll::PollOption;

use {any_port, expect_events, init_with_poll};

// TODO: test the different `PollOption`s.
// TODO: test with both ends our UdpSockets.

#[test]
fn socket_send_to() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let mut buf = [0; 20];
        let (n, addr) = std_socket.recv_from(&mut buf).unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf[0..n], b"hello world");
        addr
    });

    let mut socket = UdpSocket::bind(any_port()).unwrap();
    let local_addr = socket.local_addr().unwrap();

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    let n = socket.send_to(b"hello world", peer_addr).unwrap();
    assert_eq!(n, 11);
    assert!(socket.take_error().unwrap().is_none());

    assert_eq!(t.join().unwrap(), local_addr);
}

#[test]
fn socket_peek_and_receive_from() {
    let mut socket = UdpSocket::bind(any_port()).unwrap();
    let addr = socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let std_socket = net::UdpSocket::bind(any_port()).unwrap();
        let n = std_socket.send_to(b"hello world", addr).unwrap();
        assert_eq!(n, 11);
        std_socket.local_addr().unwrap()
    });

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Should be able to receive the datagram send by the other thread.
    let (mut buf1, mut buf2) = ([0; 20], [0; 20]);
    let (n1, addr1) = socket.peek_from(&mut buf1).unwrap();
    let (n2, addr2) = socket.recv_from(&mut buf2).unwrap();

    // Make sure the peeked and received values are the same.
    assert_eq!(n1, n2);
    assert_eq!(addr1, addr2);
    assert_eq!(buf1, buf2);

    // And check if both of them are the same as we expect.
    assert_eq!(n1, 11);
    assert_eq!(&buf1[0..n1], b"hello world");

    // Now it should return would block errors, rather the blocking.
    assert!(socket.recv_from(&mut buf1).unwrap_err().kind() == io::ErrorKind::WouldBlock);
    assert!(socket.take_error().unwrap().is_none());

    assert_eq!(t.join().unwrap(), addr1);
}

#[test]
fn socket_reregister() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let mut buf = [0; 20];
        let (n, _) = std_socket.recv_from(&mut buf).unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf[0..n], b"hello world");
    });

    let mut socket = UdpSocket::bind(any_port()).unwrap();

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE | Ready::READABLE, PollOption::Edge).unwrap();

    // Reregister the socket.
    poll.reregister(&mut socket, EventedId(2), Ready::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(2), Ready::WRITABLE),
    ]);

    let n = socket.send_to(b"hello world", peer_addr).unwrap();
    assert_eq!(n, 11);
    assert!(socket.take_error().unwrap().is_none());

    t.join().unwrap();
}

#[test]
fn socket_deregister() {
    let mut socket = UdpSocket::bind(any_port()).unwrap();
    let addr = socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let std_socket = net::UdpSocket::bind(any_port()).unwrap();
        let n = std_socket.send_to(b"hello world", addr).unwrap();
        assert_eq!(n, 11);
    });

    let (mut poll, mut events) = init_with_poll();

    let mut socket = UdpSocket::bind(any_port()).unwrap();

    // Register and deregister the socket.
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE | Ready::READABLE, PollOption::Edge).unwrap();
    poll.deregister(&mut socket).unwrap();

    // Now we shouldn't receive any events.
    poll.poll(&mut events, Some(Duration::from_millis(200))).unwrap();
    assert_eq!(events.len(), 0, "didn't expect any event, but got {:?}", events);

    t.join().unwrap();
}

#[test]
fn connected_socket_send() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let mut buf = [0; 20];
        let (n, addr) = std_socket.recv_from(&mut buf).unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf[0..n], b"hello world");
        addr
    });

    let mut socket = ConnectedUdpSocket::connect(any_port(), peer_addr).unwrap();
    let local_addr = socket.local_addr().unwrap();

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    let n = socket.send(b"hello world").unwrap();
    assert_eq!(n, 11);
    assert!(socket.take_error().unwrap().is_none());

    assert_eq!(t.join().unwrap(), local_addr);
}

#[test]
fn connected_socket_peek_and_receive() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();

    let mut socket = ConnectedUdpSocket::connect(any_port(), peer_addr).unwrap();
    let addr = socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let n = std_socket.send_to(b"hello world", addr).unwrap();
        assert_eq!(n, 11);
    });

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::READABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Should be able to receive the datagram send by the other thread.
    let (mut buf1, mut buf2) = ([0; 20], [0; 20]);
    let n1 = socket.peek(&mut buf1).unwrap();
    let n2 = socket.recv(&mut buf2).unwrap();

    // Make sure the peeked and received values are the same.
    assert_eq!(n1, n2);
    assert_eq!(buf1, buf2);

    // And check if both of them are the same as we expect.
    assert_eq!(n1, 11);
    assert_eq!(&buf1[0..n1], b"hello world");

    // Now it should return would block errors, rather the blocking.
    assert!(socket.recv(&mut buf1).unwrap_err().kind() == io::ErrorKind::WouldBlock);
    assert!(socket.take_error().unwrap().is_none());

    t.join().unwrap();
}

#[test]
fn connected_socket_reregister() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let mut buf = [0; 20];
        let (n, _) = std_socket.recv_from(&mut buf).unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf[0..n], b"hello world");
    });

    let mut socket = ConnectedUdpSocket::connect(any_port(), peer_addr).unwrap();

    let (mut poll, mut events) = init_with_poll();
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE | Ready::READABLE, PollOption::Edge).unwrap();

    // Reregister the socket.
    poll.reregister(&mut socket, EventedId(2), Ready::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(2), Ready::WRITABLE),
    ]);

    let n = socket.send(b"hello world").unwrap();
    assert_eq!(n, 11);
    assert!(socket.take_error().unwrap().is_none());

    t.join().unwrap();
}

#[test]
fn connected_socket_deregister() {
    let std_socket = net::UdpSocket::bind(any_port()).unwrap();
    let peer_addr = std_socket.local_addr().unwrap();
    let mut socket = ConnectedUdpSocket::connect(any_port(), peer_addr).unwrap();
    let addr = socket.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let n = std_socket.send_to(b"hello world", addr).unwrap();
        assert_eq!(n, 11);
    });

    let (mut poll, mut events) = init_with_poll();

    // Register and deregister the socket.
    poll.register(&mut socket, EventedId(0), Ready::WRITABLE | Ready::READABLE, PollOption::Edge).unwrap();
    poll.deregister(&mut socket).unwrap();

    // Now we shouldn't receive any events.
    poll.poll(&mut events, Some(Duration::from_millis(200))).unwrap();
    assert_eq!(events.len(), 0, "didn't expect any event, but got {:?}", events);

    t.join().unwrap();
}
