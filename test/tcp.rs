use std::thread;
use std::io::{self, Read, Write};
use std::net::{self, SocketAddr, Shutdown};
use std::time::Duration;
use std::collections::HashMap;

use mio_st::event::{Event, EventedId};
use mio_st::net::{TcpListener, TcpStream};
use mio_st::poll::{Poll, PollOpt, Ready};

use {expect_events, init_with_poll};

// TODO: add tests for both TcpStream and TcpListener:
// reregistering and
// deregistering.

#[test]
fn listener() {
    let mut listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let stream = net::TcpStream::connect(addr).unwrap();
        drop(stream);
    });

    let (mut poll, mut events) = init_with_poll(8);

    poll.register(&mut listener, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Should be able to accept a single connection, created in the other thread
    // and then it should return would block errors.
    assert!(listener.accept().is_ok());
    assert!(listener.accept().unwrap_err().kind() == io::ErrorKind::WouldBlock);
    assert!(listener.take_error().unwrap().is_none());

    t.join().unwrap();
}

#[test]
fn listener_bind_twice() {
    let mut listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    assert!(TcpListener::bind(addr).is_err());
}

#[test]
fn deregistered_listener() {
    let (mut poll, mut events) = init_with_poll(8);

    let mut listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();

    poll.register(&mut listener, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut listener).unwrap();

    poll.poll(&mut events, Some(Duration::from_secs(1))).unwrap();
    for event in &mut events {
        assert_ne!(event.id(), EventedId(0), "received an event for a deregistered listener");
    }
}

#[test]
fn listener_poll_opt() {
    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.

    let mut level_listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let level_addr = level_listener.local_addr().unwrap();
    let t1 = thread::spawn(move || two_connections(level_addr));

    let mut edge_listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let edge_addr = edge_listener.local_addr().unwrap();
    let t2 = thread::spawn(move || two_connections(edge_addr));

    let mut oneshot_listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let oneshot_addr = oneshot_listener.local_addr().unwrap();
    let t3 = thread::spawn(move || two_connections(oneshot_addr));

    let (mut poll, mut events) = init_with_poll(16);

    const LEVEL_ID: EventedId = EventedId(0);
    const EDGE_ID: EventedId = EventedId(1);
    const ONESHOT_ID: EventedId = EventedId(2);

    poll.register(&mut level_listener, LEVEL_ID, Ready::READABLE, PollOpt::Level).unwrap();
    poll.register(&mut edge_listener, EDGE_ID, Ready::READABLE, PollOpt::Edge).unwrap();
    poll.register(&mut oneshot_listener, ONESHOT_ID, Ready::READABLE, PollOpt::Oneshot).unwrap();

    // What events we've seen so far.
    let mut seen_level = 0;
    let mut seen_edge = 0;
    let mut seen_oneshot = false;

    let mut another_loop: isize = 3;
    while another_loop > 0 {
        poll.poll(&mut events, None).unwrap();

        for event in &mut events {
            match event.id() {
                LEVEL_ID => {
                    if seen_level == 1 {
                        level_listener.accept().unwrap();
                    }
                    seen_level += 1;
                },
                EDGE_ID => {
                    edge_listener.accept().unwrap();
                    seen_edge += 1;
                },
                ONESHOT_ID => {
                    if seen_oneshot {
                        panic!("got a second event with oneshot poll option");
                    } else {
                        seen_oneshot = true;
                    }
                },
                _ => unreachable!(),
            }
        }

        if seen_level >= 3 && seen_edge == 2 && seen_oneshot {
            // We we're done we still run another iteration to make sure we get
            // another event for the level stream.
            another_loop -= 1;
        }
    }

    t1.join().unwrap();
    t2.join().unwrap();
    t3.join().unwrap();
}

fn two_connections(addr: SocketAddr) {
    let s1 = net::TcpStream::connect(addr).unwrap();
    let s2 = net::TcpStream::connect(addr).unwrap();
    drop(s1);
    drop(s2);
}

#[test]
fn stream() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = net::TcpListener::bind(addr).unwrap();
    let addr = listener.local_addr().unwrap();

    const MSG: &[u8; 11] = b"Hello world";

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();

        stream.write_all(MSG).unwrap();

        let mut buf = [0; 20];
        let n = stream.read(&mut buf[0..19]).unwrap();
        assert_eq!(&buf[0..n], &*MSG);
    });

    let (mut poll, mut events) = init_with_poll(8);

    let mut stream = TcpStream::connect(addr).unwrap();
    poll.register(&mut stream, EventedId(0), Ready::all(), PollOpt::Level).unwrap();

    thread::sleep(Duration::from_millis(10));
    expect_events(&mut poll, &mut events, 2, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    assert_eq!(stream.peer_addr().unwrap(), addr);

    let keep_alive = stream.keepalive().unwrap();
    if keep_alive.is_some() {
        stream.set_keepalive(None).unwrap();
        assert_eq!(stream.keepalive().unwrap(), None);
    } else {
        stream.set_keepalive(Some(Duration::from_secs(1))).unwrap();
        assert_eq!(stream.keepalive().unwrap(), Some(Duration::from_secs(1)));
    }

    let mut buf = [0; 20];
    let capacity = 19;
    let n = stream.peek(&mut buf[0..capacity]).unwrap();
    assert_eq!(&buf[0..n], &*MSG);

    let n = stream.read(&mut buf[0..capacity]).unwrap();
    assert_eq!(&buf[0..n], &*MSG);

    stream.shutdown(Shutdown::Read).unwrap();
    // FIXME: shutdown doesn't seem to work.
    //assert_eq!(stream.read(&mut buf[0..capacity]).unwrap_err().kind(), io::ErrorKind::NotConnected);

    stream.write_all(MSG).unwrap();
    stream.flush().unwrap();

    stream.shutdown(Shutdown::Write).unwrap();
    // FIXME: shutdown doesn't seem to work.
    //assert_eq!(stream.write(&buf[0..capacity]).unwrap_err().kind(), io::ErrorKind::NotConnected);

    assert!(stream.take_error().unwrap().is_none());

    t.join().unwrap();
}

#[test]
fn stream_poll_opt() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = net::TcpListener::bind(addr).unwrap();
    let addr = listener.local_addr().unwrap();

    // Fork before creating poll, because doing it after creating poll will
    // result in undefined behaviour.
    let t = thread::spawn(move || {
        let mut count = 3;
        for res in listener.incoming() {
            let mut stream = res.unwrap();
            stream.write_all(b"hello").unwrap();
            count -= 1;
            if count <= 0 {
                return
            }
        }
    });

    let (mut poll, mut events) = init_with_poll(16);

    const LEVEL_ID: EventedId = EventedId(0);
    const EDGE_ID: EventedId = EventedId(1);
    const ONESHOT_ID: EventedId = EventedId(2);

    let mut level_stream = TcpStream::connect(addr).unwrap();
    poll.register(&mut level_stream, LEVEL_ID, Ready::all(), PollOpt::Level).unwrap();
    let mut edge_stream = TcpStream::connect(addr).unwrap();
    poll.register(&mut edge_stream, EDGE_ID, Ready::all(), PollOpt::Edge).unwrap();
    let mut oneshot_stream = TcpStream::connect(addr).unwrap();
    poll.register(&mut oneshot_stream, ONESHOT_ID, Ready::all(), PollOpt::Oneshot).unwrap();

    // What events we've seen so far.
    let mut seen_level = false;
    let mut seen_edge = false;
    let mut seen_oneshot = false;

    let mut another_loop: isize = 3;
    while another_loop > 0 {
        poll.poll(&mut events, None).unwrap();

        let mut seen_level_loop = false;

        for event in &mut events {
            match event.id() {
                LEVEL_ID => {
                    seen_level = true;
                    seen_level_loop = true;
                },
                EDGE_ID => {
                    seen_edge = true;
                },
                ONESHOT_ID => {
                    if seen_oneshot {
                        panic!("got a second event with oneshot poll option");
                    } else {
                        seen_oneshot = true;
                    }
                },
                _ => unreachable!(),
            }
        }

        assert!(seen_level_loop, "didn't see level event this iteration");

        if seen_level && seen_edge && seen_oneshot {
            // We we're done we still run another iteration to make sure we get
            // another event for the level stream.
            another_loop -= 1;
        }
    }

    t.join().unwrap();
}

#[test]
fn listener_and_stream() {
    let (mut poll, mut events) = init_with_poll(16);

    const LISTENER_ID: EventedId = EventedId(0);
    let mut connections = HashMap::with_capacity(4);
    let mut current_id = 1;

    let mut listener = TcpListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    poll.register(&mut listener, LISTENER_ID, Ready::READABLE, PollOpt::Edge).unwrap();

    let stream1 = TcpStream::connect(addr).unwrap();
    add_connection(&mut connections, &mut poll, Connection::new(stream1, ConnectionType::Connected), &mut current_id);
    let stream2 = TcpStream::connect(addr).unwrap();
    add_connection(&mut connections, &mut poll, Connection::new(stream2, ConnectionType::Connected), &mut current_id);

    loop {
        poll.poll(&mut events, None).unwrap();

        for event in &mut events {
            match event.id() {
                LISTENER_ID => loop {
                    let stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                        Err(err) => panic!("unexpected error: {}", err),
                    };

                    let connection = Connection::new(stream, ConnectionType::Accepted);
                    add_connection(&mut connections, &mut poll, connection, &mut current_id);
                },
                id => {
                    let remove = {
                        // In the same loop we can receive events for the same
                        // connection that we removed in the previous iteration.
                        connections.get_mut(&id)
                            .map(|conn| conn.progress())
                            .unwrap_or(false)
                    };
                    if remove {
                        let mut connection = connections.remove(&id).unwrap();
                        poll.deregister(&mut connection.stream).unwrap();
                    }
                },
            }
        }

        if connections.is_empty() {
            break;
        }
    }
}

#[derive(Debug)]
struct Connection {
    stream: TcpStream,
    ttype: ConnectionType,
    stage: usize,
}

#[derive(Copy, Clone, Debug)]
enum ConnectionType { Connected, Accepted }

impl Connection {
    fn new(stream: TcpStream, ttype: ConnectionType) -> Connection {
        Connection { stream, ttype, stage: 0 }
    }

    /// Returns true if the connection is done.
    fn progress(&mut self) -> bool {
        const MSG_CONNECTED: &[u8; 9] = b"connceted";
        const MSG_ACCEPTED: &[u8; 8] = b"accepted";

        match (self.stage, self.ttype) {
            (0, ConnectionType::Connected) => {
                match self.stream.write_all(&*MSG_CONNECTED) {
                    Ok(()) => {},
                    Err(ref err) if is_would_block(err) => return false,
                    Err(err) => panic!("unexpected error: {}", err),
                }
            },
            (1, ConnectionType::Connected) => {
                let mut buf = [0; 20];
                match self.stream.read(&mut buf) {
                    Ok(n) => {
                        assert_eq!(&buf[0..n], &*MSG_ACCEPTED);
                    },
                    Err(ref err) if is_would_block(err) => return false,
                    Err(err) => panic!("unexpected error: {}", err),
                }
            },
            (0, ConnectionType::Accepted) => {
                let mut buf = [0; 20];
                match self.stream.read(&mut buf) {
                    Ok(n) => {
                        assert_eq!(&buf[0..n], &*MSG_CONNECTED);
                    },
                    Err(ref err) if is_would_block(err) => return false,
                    Err(err) => panic!("unexpected error: {}", err),
                }
            },
            (1, ConnectionType::Accepted) => {
                match self.stream.write_all(&*MSG_ACCEPTED) {
                    Ok(()) => {},
                    Err(ref err) if is_would_block(err) => return false,
                    Err(err) => panic!("unexpected error: {}", err),
                }
            },
            (2, _) => return true,
            _ => unreachable!(),
        }

        self.stage += 1;
        self.progress()
    }
}

fn is_would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn add_connection(connections: &mut HashMap<EventedId, Connection>, poll: &mut Poll, mut connection: Connection, current_id: &mut usize) {
    let id = EventedId(*current_id);
    poll.register(&mut connection.stream, id, Ready::all(), PollOpt::Edge).unwrap();
    connections.insert(id, connection);
    *current_id += 1;
}
