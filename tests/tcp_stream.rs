use std::io::{self, Read, Write};
use std::net::{self, SocketAddr,Shutdown};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::mpsc::channel;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::net::TcpStream;
use mio_st::poll::{Interests, PollOption};

mod util;

use self::util::{any_local_address, assert_would_block, expect_events, init, init_with_poller};

/// Data used in reading and writing tests.
const DATA: &'static [u8; 12] = b"Hello world!";

const ID: EventedId = EventedId(0);
const ID2: EventedId = EventedId(1);

#[test]
fn tcp_stream() {
    let (mut poller, mut events) = init_with_poller();

    let (sender, receiver) = channel();

    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let listener_address = listener.local_addr().unwrap();
        sender.send(listener_address).unwrap();

        let (stream, address) = listener.accept().unwrap();
        assert_eq!(stream.local_addr().unwrap(), listener_address);

        let peer_address = stream.peer_addr().unwrap();
        assert_eq!(peer_address, address);
        sender.send(peer_address).unwrap();
    });

    let listener_address = receiver.recv().unwrap();
    let mut stream = TcpStream::connect(listener_address).unwrap();

    poller.register(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)
        .expect("unable to register TCP stream");

    // Connect is non-blocking, so wait until the other thread accepted the
    // connection at which point we should receive an event that the connection
    // is ready.
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);
    assert_eq!(stream.peer_addr().unwrap(), listener_address);

    let accepted_peer_address = receiver.recv().unwrap();
    assert_eq!(stream.local_addr().unwrap(), accepted_peer_address);
    assert!(stream.take_error().unwrap().is_none());

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_ttl() {
    init();

    let (thread_handle, address) = start_listener(1, None);

    let mut stream = TcpStream::connect(address).unwrap();

    const TTL: u32 = 10;
    stream.set_ttl(TTL).unwrap();
    assert_eq!(stream.ttl().unwrap(), TTL);
    assert!(stream.take_error().unwrap().is_none());

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_nodelay() {
    init();

    let (thread_handle, address) = start_listener(1, None);

    let mut stream = TcpStream::connect(address).unwrap();

    const NO_DELAY: bool = true;
    stream.set_nodelay(NO_DELAY).unwrap();
    assert_eq!(stream.nodelay().unwrap(), NO_DELAY);
    assert!(stream.take_error().unwrap().is_none());

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_peek() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        stream.write(DATA).unwrap();

        drop(stream);
        barrier2.wait();
    });

    let address = receiver.recv().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();

    poller.register(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let n = stream.peek(&mut buf).unwrap();
    assert_eq!(n, DATA.len());
    assert_eq!(buf[0..n], DATA[..]);

    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, DATA.len());
    assert_eq!(buf[0..n], DATA[..]);

    // Make sure the connection is dropped.
    barrier.wait();
    assert_eq!(stream.read(&mut buf).unwrap(), 0);

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_shutdown_read() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let (thread_handle, address) = start_listener(1, Some(barrier.clone()));

    let mut stream = TcpStream::connect(address).unwrap();

    poller.reregister(&mut stream, EventedId(0), Interests::WRITABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    stream.shutdown(Shutdown::Read).unwrap();
    let mut buf = [0; 20];
    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, 0);

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_shutdown_write() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let (thread_handle, address) = start_listener(1, Some(barrier.clone()));

    let mut stream = TcpStream::connect(address).unwrap();

    poller.reregister(&mut stream, EventedId(0), Interests::WRITABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    stream.shutdown(Shutdown::Write).unwrap();
    let mut buf = [1; 5];
    let err = stream.write(&mut buf).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_shutdown_both() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let (thread_handle, address) = start_listener(1, Some(barrier.clone()));

    let mut stream = TcpStream::connect(address).unwrap();

    poller.reregister(&mut stream, EventedId(0), Interests::WRITABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    stream.shutdown(Shutdown::Both).unwrap();

    let mut buf = [0; 5];
    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, 0);

    let err = stream.write(&mut buf).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_read() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.reregister(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE),
    ]);

    // Should read the byte written.
    let mut buf = [0; 20];
    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, DATA.len());
    assert_eq!(buf[0..n], DATA[..]);

    // Stream should be non-blocking if no data is available.
    assert_would_block(stream.read(&mut buf));

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

// TODO: add test to check that writing is non-blocking.
#[test]
fn tcp_stream_write() {
    let (mut poller, mut events) = init_with_poller();

    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 20];
        let n = stream.read(&mut buf).unwrap();
        assert_eq!(n, DATA.len());
        assert_eq!(buf[0..n], DATA[..]);
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.reregister(&mut stream, EventedId(0), Interests::WRITABLE, PollOption::Edge)
        .expect("unable to register TCP stream");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::WRITABLE),
    ]);

    assert_eq!(stream.write(DATA).unwrap(), DATA.len());
    stream.flush().unwrap();

    // Unblock the thread.
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_raw_fd() {
    init();

    let (thread_handle, address) = start_listener(1, None);

    let mut stream = TcpStream::connect(address).unwrap();
    let address = stream.local_addr().unwrap();

    let raw_fd1 = stream.as_raw_fd();
    let raw_fd2 = stream.into_raw_fd();
    assert_eq!(raw_fd1, raw_fd2);

    let mut stream = unsafe { TcpStream::from_raw_fd(raw_fd2) };
    assert_eq!(stream.as_raw_fd(), raw_fd1);
    assert_eq!(stream.local_addr().unwrap(), address);

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_deregister() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let (thread_handle, address) = start_listener(1, Some(barrier.clone()));

    let mut stream = TcpStream::connect(address).unwrap();

    poller.register(&mut stream, EventedId(0), TcpStream::INTERESTS, PollOption::Edge).unwrap();
    poller.deregister(&mut stream).unwrap();

    // Shouldn't get any events after deregistering.
    poller.poll(&mut events, Some(Duration::from_millis(500))).unwrap();
    assert!(events.is_empty());

    // But we do expect to be connected.
    assert_eq!(stream.peer_addr().unwrap(), address);

    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_reregister() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let (thread_handle, address) = start_listener(1, Some(barrier.clone()));

    let mut stream = TcpStream::connect(address).unwrap();

    poller.register(&mut stream, EventedId(0), Interests::WRITABLE, PollOption::Edge).unwrap();
    poller.deregister(&mut stream).unwrap();
    poller.reregister(&mut stream, EventedId(1), Interests::WRITABLE, PollOption::Edge).unwrap();

    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(1), Ready::WRITABLE),
    ]);

    assert_eq!(stream.peer_addr().unwrap(), address);

    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_edge_poll_option_drain() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();

        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.reregister(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)
        .expect("unable to register TCP stream");

    let mut seen_events = 0;
    for _ in 0..4  {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if seen_events == 0 => {
                    let mut buf = [0; 20];
                    assert_eq!(stream.read(&mut buf).unwrap(), DATA.len());
                    assert_would_block(stream.read(&mut buf));
                    seen_events = 1;

                    // Unblock second write.
                    barrier.wait();
                },
                ID if seen_events == 1 => {
                    let mut buf = [0; 20];
                    assert_eq!(stream.read(&mut buf).unwrap(), DATA.len());
                    assert_would_block(stream.read(&mut buf));
                    seen_events = 2;
                },
                ID => panic!("unexpected event for level TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_edge_poll_option_no_drain() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.reregister(&mut stream, EventedId(0), Interests::READABLE, PollOption::Edge)
        .expect("unable to register TCP stream");

    let mut seen_event = false;
    for _ in 0..3  {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if !seen_event => {
                    // Don't read the entire buffer, only half. Then we
                    // shouldn't see any more events.
                    let mut buf = [0; 6];
                    assert_eq!(stream.read(&mut buf).unwrap(), 6);
                    seen_event = true;
                },
                ID => panic!("unexpected event for level TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_level_poll_option() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.reregister(&mut stream, EventedId(0), Interests::READABLE, PollOption::Level)
        .expect("unable to register TCP stream");

    let mut seen_events = 0;
    for _ in 0..3  {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if seen_events == 0 => {
                    // Don't read the entire buffer, only half. Then we
                    // should see another event.
                    let mut buf = [0; 6];
                    assert_eq!(stream.read(&mut buf).unwrap(), 6);
                    seen_events = 1;
                },
                ID if seen_events == 1 => {
                    // Read the other half of the message.
                    let mut buf = [0; 6];
                    assert_eq!(stream.read(&mut buf).unwrap(), 6);
                    assert_would_block(stream.read(&mut buf));
                    seen_events = 2;
                },
                ID => panic!("unexpected event for level TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    // Unblock the thread.
    barrier.wait();
    thread_handle.join().expect("unable to join thread");
}

#[test]
fn tcp_stream_oneshot_poll_option() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();

        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.register(&mut stream, ID, Interests::READABLE, PollOption::Oneshot).unwrap();

    let mut seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if !seen_event => seen_event = true,
                ID => panic!("unexpected event for oneshot TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    barrier.wait();
    thread_handle.join().unwrap();
}

#[test]
fn tcp_stream_oneshot_poll_option_reregister() {
    let (mut poller, mut events) = init_with_poller();

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        let (mut stream, _) = listener.accept().unwrap();

        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();

        assert_eq!(stream.write(DATA).unwrap(), DATA.len());
        barrier2.wait();
    });
    let address = receiver.recv().unwrap();

    let mut stream = TcpStream::connect(address).unwrap();
    poller.register(&mut stream, ID, Interests::READABLE, PollOption::Oneshot).unwrap();

    let mut seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID if !seen_event => seen_event = true,
                ID => panic!("unexpected event for oneshot TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    // Unblock the second write.
    barrier.wait();

    // Reregister the listener and we expect to see more events.
    poller.reregister(&mut stream, ID2, Interests::READABLE, PollOption::Oneshot).unwrap();

    seen_event = false;
    for _ in 0..2 {
        poller.poll(&mut events, Some(Duration::from_millis(100))).unwrap();

        for event in &mut events {
            match event.id() {
                ID2 if !seen_event => seen_event = true,
                ID2 => panic!("unexpected event for oneshot TCP stream"),
                _ => unreachable!(),
            }
        }
    }

    barrier.wait();
    thread_handle.join().unwrap();
}

/// Start a listener that accepts `n_connections` connections on the returned
/// address. It optionally calls the provided function with the stream.
fn start_listener(n_connections: usize, barrier: Option<Arc<Barrier>>) -> (thread::JoinHandle<()>, SocketAddr) {
    let (sender, receiver) = channel();
    let thread_handle = thread::spawn(move || {
        let listener = net::TcpListener::bind(any_local_address()).unwrap();
        let local_address = listener.local_addr().unwrap();
        sender.send(local_address).unwrap();

        for _ in 0..n_connections {
            let (stream, _) = listener.accept().unwrap();
            if let Some(ref barrier) = barrier {
                barrier.wait();
            }
            drop(stream);
        }
    });
    (thread_handle, receiver.recv().unwrap())
}
