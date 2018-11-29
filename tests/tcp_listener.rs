use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::{Arc, Barrier};
use std::thread::{self, sleep};
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::net::TcpListener;
use mio_st::poll::{Interests, PollOption, Poller};

mod util;

use self::util::{any_local_address, assert_would_block, expect_events, init, init_with_poller};

#[test]
fn tcp_listener() {
    let (mut poller, mut events) = init_with_poller();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let address = listener.local_addr().unwrap();

    poller.register(&mut listener, EventedId(0), TcpListener::INTERESTS, PollOption::Edge)
        .expect("unable to register TCP listener");

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Expect a single connection.
    let (mut stream, peer_address) = listener.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections.
    assert_would_block(listener.accept());

    assert!(listener.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_ipv6() {
    let (mut poller, mut events) = init_with_poller();

    let address: SocketAddr = "[::1]:0".parse().unwrap();
    assert!(address.is_ipv6());
    let mut listener = TcpListener::bind(address).unwrap();
    let address = listener.local_addr().unwrap();

    poller.register(&mut listener, EventedId(0), TcpListener::INTERESTS, PollOption::Edge)
        .expect("unable to register TCP listener");

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Expect a single connection.
    let (mut stream, peer_address) = listener.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections.
    assert_would_block(listener.accept());

    assert!(listener.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_try_clone_same_poller() {
    let (mut poller, mut events) = init_with_poller();

    // Cloning a listener should result in different file descriptors with the
    // same local address.
    let mut listener1 = TcpListener::bind(any_local_address()).unwrap();
    let mut listener2 = listener1.try_clone().expect("unable to clone TCP listener");
    assert_ne!(listener1.as_raw_fd(), listener2.as_raw_fd());
    let address = listener1.local_addr().unwrap();
    assert_eq!(address, listener2.local_addr().unwrap());

    // Should be able to register both listeners with the same poller.
    poller.register(&mut listener1, EventedId(0), TcpListener::INTERESTS, PollOption::Edge).unwrap();
    poller.register(&mut listener2, EventedId(1), TcpListener::INTERESTS, PollOption::Edge).unwrap();

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    // We should have events for both listeners.
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
        Event::new(EventedId(1), Ready::READABLE),
    ]);

    // Expect a single connection on 1 of the listeners.
    let (mut stream, peer_address) = listener1.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections on either listeners.
    assert_would_block(listener2.accept());
    assert_would_block(listener1.accept());

    assert!(listener1.take_error().unwrap().is_none());
    assert!(listener2.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_try_clone_different_poller() {
    let (mut poller1, mut events) = init_with_poller();
    let mut poller2 = Poller::new().unwrap();

    // Cloning a listener should result in different file descriptors with the
    // same local address.
    let mut listener1 = TcpListener::bind(any_local_address()).unwrap();
    let mut listener2 = listener1.try_clone().expect("unable to clone TCP listener");
    assert_ne!(listener1.as_raw_fd(), listener2.as_raw_fd());
    let address = listener1.local_addr().unwrap();
    assert_eq!(address, listener2.local_addr().unwrap());

    // Should be able to register both listeners with the same poller.
    poller1.register(&mut listener1, EventedId(0), TcpListener::INTERESTS, PollOption::Edge).unwrap();
    poller2.register(&mut listener2, EventedId(0), TcpListener::INTERESTS, PollOption::Edge).unwrap();

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    // Both pollers should have received an event.
    expect_events(&mut poller1, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);
    expect_events(&mut poller2, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Expect a single connection on 1 of the listeners.
    let (mut stream, peer_address) = listener2.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections on either listeners.
    assert_would_block(listener2.accept());
    assert_would_block(listener1.accept());

    assert!(listener1.take_error().unwrap().is_none());
    assert!(listener2.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_ttl() {
    init();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();

    const TTL: u32 = 10;
    listener.set_ttl(TTL).unwrap();
    assert_eq!(listener.ttl().unwrap(), TTL);
    assert!(listener.take_error().unwrap().is_none());
}

#[test]
fn tcp_listener_raw_fd() {
    init();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let address = listener.local_addr().unwrap();

    let raw_fd1 = listener.as_raw_fd();
    let raw_fd2 = listener.into_raw_fd();
    assert_eq!(raw_fd1, raw_fd2);

    let mut listener = unsafe { TcpListener::from_raw_fd(raw_fd2) };
    assert_eq!(listener.as_raw_fd(), raw_fd1);
    assert_eq!(listener.local_addr().unwrap(), address);
}

#[test]
fn tcp_listener_deregister() {
    let (mut poller, mut events) = init_with_poller();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let address = listener.local_addr().unwrap();

    poller.register(&mut listener, EventedId(0), TcpListener::INTERESTS, PollOption::Edge).unwrap();
    poller.deregister(&mut listener).unwrap();

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    // Shouldn't get any events after deregistering.
    poller.poll(&mut events, Some(Duration::from_millis(500))).unwrap();
    assert!(events.is_empty());

    // But we do expect a single connection, even without an event.
    let (mut stream, peer_address) = listener.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections.
    assert_would_block(listener.accept());

    assert!(listener.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_reregister() {
    let (mut poller, mut events) = init_with_poller();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let address = listener.local_addr().unwrap();

    poller.register(&mut listener, EventedId(0), TcpListener::INTERESTS, PollOption::Edge).unwrap();
    poller.reregister(&mut listener, EventedId(1), TcpListener::INTERESTS, PollOption::Edge).unwrap();

    // Start another thread that connects to our listener.
    let thread_handle = thread::spawn(move || {
        let stream = net::TcpStream::connect(address).unwrap();
        drop(stream);
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(1), Ready::READABLE),
    ]);

    // Expect a single connection.
    let (mut stream, peer_address) = listener.accept()
        .expect("unable to accept connection");
    assert!(peer_address.ip().is_loopback());
    assert_eq!(stream.peer_addr().unwrap(), peer_address);
    assert_eq!(stream.local_addr().unwrap(), address);

    // Expect no more connections.
    assert_would_block(listener.accept());

    assert!(listener.take_error().unwrap().is_none());
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_listener_edge_poll_option_drain() {
    let (mut poller, mut events) = init_with_poller();

    const ID: EventedId = EventedId(0);
    // Wait after first connection is made, to allow this test to hit
    // `WouldBlock` error when accepting after this first poll.
    let barrier = Arc::new(Barrier::new(2));

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let thread_handle1 = start_connections(&mut listener, 1, None);
    let thread_handle2 = start_connections(&mut listener, 2, Some(barrier.clone()));
    poller.register(&mut listener, ID, TcpListener::INTERESTS, PollOption::Edge).unwrap();

    // Give the connections some time to run.
    sleep(Duration::from_millis(100));

    let mut seen_edge = 0;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if seen_edge == 0 => {
                    // After the first call to poll we expect 2 connections to
                    // be ready.
                    assert!(listener.accept().is_ok());
                    assert!(listener.accept().is_ok());
                    assert_would_block(listener.accept());

                    // Unblock the second connection.
                    barrier.wait();

                    seen_edge += 2;
                },
                ID if seen_edge == 2 => {
                    // After the second poll we expect 1 more connection to be
                    // ready.
                    assert!(listener.accept().is_ok());
                    assert_would_block(listener.accept());
                    seen_edge += 1;

                    // Unblock the connection thread.
                    barrier.wait();
                }
                ID => panic!("unexpected event for edge TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    thread_handle1.join().unwrap();
    thread_handle2.join().unwrap();
}

#[test]
fn tcp_listener_edge_poll_option_no_drain() {
    let (mut poller, mut events) = init_with_poller();

    const ID: EventedId = EventedId(0);

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let thread_handle1 = start_connections(&mut listener, 1, None);
    let thread_handle2 = start_connections(&mut listener, 1, None);
    poller.register(&mut listener, ID, TcpListener::INTERESTS, PollOption::Edge).unwrap();

    // Give the connections some time to run.
    sleep(Duration::from_millis(100));

    let mut seen_edge = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                // Here we also expect 2 connections to be ready after the first
                // poll. We'll only accept one connection and then we don't
                // expect any more events, since we didn't drain the queue of
                // ready connections at the listener.
                ID if !seen_edge => {
                    assert!(listener.accept().is_ok());
                    seen_edge = true;
                },
                ID => panic!("unexpected event for edge TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    thread_handle1.join().unwrap();
    thread_handle2.join().unwrap();
}

#[test]
fn tcp_listener_level_poll_option() {
    let (mut poller, mut events) = init_with_poller();

    const ID: EventedId = EventedId(0);

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let thread_handle1 = start_connections(&mut listener, 2, None);
    let thread_handle2 = start_connections(&mut listener, 2, None);
    poller.register(&mut listener, ID, TcpListener::INTERESTS, PollOption::Level).unwrap();

    // Give the connections some time to run.
    sleep(Duration::from_millis(100));

    let mut seen_events = 0;
    for _ in 0..5  {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if seen_events < 4 => {
                    // More then 1 connection should be ready at a time, but
                    // we'll only accept 1 at a time. But since we're using
                    // level notifications we should keep receiving events
                    // for the other 3 connections.
                    assert!(listener.accept().is_ok());
                    seen_events += 1;
                },
                ID => panic!("unexpected event for level TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    thread_handle1.join().unwrap();
    thread_handle2.join().unwrap();
}

#[test]
fn tcp_listener_oneshot_poll_option() {
    let (mut poller, mut events) = init_with_poller();

    const ID: EventedId = EventedId(0);

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let thread_handle = start_connections(&mut listener, 2, None);
    poller.register(&mut listener, ID, TcpListener::INTERESTS, PollOption::Oneshot).unwrap();

    // Give the connections some time to run.
    sleep(Duration::from_millis(20));

    let mut seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if !seen_event => seen_event = true,
                ID => panic!("unexpected event for oneshot TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    thread_handle.join().unwrap();
}

#[test]
fn tcp_listener_oneshot_poll_option_reregister() {
    let (mut poller, mut events) = init_with_poller();

    const ID: EventedId = EventedId(0);
    const ID2: EventedId = EventedId(2);

    let mut listener = TcpListener::bind(any_local_address()).unwrap();
    let thread_handle = start_connections(&mut listener, 2, None);
    poller.register(&mut listener, ID, TcpListener::INTERESTS, PollOption::Oneshot).unwrap();

    // Give the connections some time to run.
    sleep(Duration::from_millis(20));

    let mut seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if !seen_event => seen_event = true,
                ID => panic!("unexpected event for oneshot TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    // Give the second connection some time to run.
    sleep(Duration::from_millis(20));

    // Reregister the listener and we expect to see more events.
    poller.reregister(&mut listener, ID2, TcpListener::INTERESTS, PollOption::Oneshot).unwrap();

    seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID2 if !seen_event => seen_event = true,
                ID2 => panic!("unexpected event for oneshot TCP listener"),
                _ => unreachable!(),
            }
        }
    }

    thread_handle.join().unwrap();
}

#[test]
#[should_panic(expected = "TcpListener only needs readable interests")]
fn tcp_listener_writable_interests() {
    init();

    let mut listener = TcpListener::bind(any_local_address()).unwrap();

    let mut poller = Poller::new().unwrap();
    poller.register(&mut listener, EventedId(0), Interests::WRITABLE, PollOption::Level)
        .unwrap();
}

/// Start `n_connections` connections in a different thread to the provided
/// `listener`. If a `barrier` is provided it will wait on it after each
/// connection is made (and dropped).
fn start_connections(listener: &mut TcpListener, n_connections: usize, barrier: Option<Arc<Barrier>>) -> thread::JoinHandle<()> {
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        // Create `n_connections` number of connections to the listener.
        for _ in 0..n_connections {
            let conn = net::TcpStream::connect(address).unwrap();
            drop(conn);

            if let Some(ref barrier) = barrier {
                barrier.wait();
            }
        }
    })
}
