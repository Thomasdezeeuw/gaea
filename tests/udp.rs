use std::io;
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::{Arc, Barrier};
use std::thread::{self, sleep};
use std::time::Duration;

use mio_st::event::{Event, Ready};
use mio_st::net::UdpSocket;
use mio_st::os::{Interests, RegisterOption};
use mio_st::{event, poll};

mod util;

use self::util::{any_local_address, any_local_ipv6_address, assert_error, assert_would_block, expect_events, init, init_with_os_queue};

const DATA1: &'static [u8; 12] = b"Hello world!";
const DATA2: &'static [u8; 11] = b"Hello mars!";

const ID1: event::Id = event::Id(0);
const ID2: event::Id = event::Id(1);
const ID3: event::Id = event::Id(2);

#[test]
fn udp_socket() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();

    let address1 = socket1.local_addr().unwrap();
    let address2 = socket2.local_addr().unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
        Event::new(ID2, Ready::WRITABLE),
    ]);

    socket1.send_to(DATA1, address2).unwrap();
    socket2.send_to(DATA2, address1).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let (n, got_address1) = socket1.peek_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address1, address2);

    let (n, got_address2) = socket2.peek_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);
    assert_eq!(got_address2, address1);

    let (n, got_address1) = socket1.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address1, address2);

    let (n, got_address2) = socket2.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);
    assert_eq!(got_address2, address1);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn udp_socket_ipv6() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_ipv6_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_ipv6_address()).unwrap();

    let address1 = socket1.local_addr().unwrap();
    let address2 = socket2.local_addr().unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
        Event::new(ID2, Ready::WRITABLE),
    ]);

    socket1.send_to(DATA1, address2).unwrap();
    socket2.send_to(DATA2, address1).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let (n, got_address1) = socket1.peek_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address1, address2);

    let (n, got_address2) = socket2.peek_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);
    assert_eq!(got_address2, address1);

    let (n, got_address1) = socket1.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address1, address2);

    let (n, got_address2) = socket2.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);
    assert_eq!(got_address2, address1);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn connected_udp_socket() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let address1 = socket1.local_addr().unwrap();

    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();
    socket2.connect(address1).unwrap();
    let address2 = socket2.local_addr().unwrap();

    socket1.connect(address2).unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
        Event::new(ID2, Ready::WRITABLE),
    ]);

    socket1.send(DATA1).unwrap();
    socket2.send(DATA2).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let n = socket1.peek(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);

    let n = socket2.peek(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    let n = socket1.recv(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);

    let n = socket2.recv(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn connected_udp_socket_ipv6() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_ipv6_address()).unwrap();
    let address1 = socket1.local_addr().unwrap();

    let mut socket2 = UdpSocket::bind(any_local_ipv6_address()).unwrap();
    socket2.connect(address1).unwrap();
    let address2 = socket2.local_addr().unwrap();

    socket1.connect(address2).unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE)
        .expect("unable to register UDP socket");

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
        Event::new(ID2, Ready::WRITABLE),
    ]);

    socket1.send(DATA1).unwrap();
    socket2.send(DATA2).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let n = socket1.peek(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);

    let n = socket2.peek(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    let n = socket1.recv(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);

    let n = socket2.recv(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn reconnect_udp_socket() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket3 = UdpSocket::bind(any_local_address()).unwrap();

    let address1 = socket1.local_addr().unwrap();
    let address2 = socket2.local_addr().unwrap();
    let address3 = socket3.local_addr().unwrap();

    socket1.connect(address2).unwrap();
    socket2.connect(address1).unwrap();
    socket3.connect(address1).unwrap();

    os_queue.register(&mut socket1, ID1, Interests::READABLE, RegisterOption::EDGE).unwrap();
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();
    os_queue.register(&mut socket3, ID3, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID2, Ready::WRITABLE),
        Event::new(ID3, Ready::WRITABLE),
    ]);

    socket2.send(DATA1).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let n = socket1.recv(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    // Now change the address the socket is connected to and send some data from
    // that address.
    socket1.connect(address3).unwrap();
    socket3.send(DATA2).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::READABLE),
    ]);

    let n = socket1.recv(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
    assert!(socket3.take_error().unwrap().is_none());
}

#[test]
fn unconnected_udp_socket_connected_methods() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();
    let address2 = socket2.local_addr().unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();
    os_queue.register(&mut socket2, ID2, Interests::READABLE, RegisterOption::EDGE).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
    ]);

    // Socket is unconnected, but we're using an connected method.
    assert_error(socket1.send(DATA1), "address required");

    // Now send some actual data.
    socket1.send_to(DATA1, address2).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let n = socket2.peek(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    let n = socket2.recv(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn connected_udp_socket_unconnected_methods() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket3 = UdpSocket::bind(any_local_address()).unwrap();

    let address2 = socket2.local_addr().unwrap();
    let address3 = socket3.local_addr().unwrap();

    socket1.connect(address3).unwrap();
    socket3.connect(address2).unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();
    os_queue.register(&mut socket3, ID3, Interests::READABLE, RegisterOption::EDGE).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID1, Ready::WRITABLE),
        Event::new(ID2, Ready::WRITABLE),
    ]);

    // Can't use `send_to`.
    // Linux (and Android) actually allow `send_to` even if the socket is
    // connected.
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    assert_error(socket1.send_to(DATA1, address2), "already connected");
    // Even if the address is the same.
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    assert_error(socket1.send_to(DATA1, address3), "already connected");

    socket2.send_to(DATA2, address3).unwrap();

    // Ensure the events show up.
    sleep(Duration::from_millis(10));
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID3, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let (n, got_address1) = socket3.peek_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address1, address2);

    let (n, got_address2) = socket3.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA2.len());
    assert_eq!(buf[..n], DATA2[..]);
    assert_eq!(got_address2, address2);

    assert!(socket1.take_error().unwrap().is_none());
    assert!(socket2.take_error().unwrap().is_none());
}

#[test]
fn udp_socket_raw_fd() {
    init();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let raw_fd1 = socket.as_raw_fd();
    let raw_fd2 = socket.into_raw_fd();
    assert_eq!(raw_fd1, raw_fd2);

    let mut socket = unsafe { UdpSocket::from_raw_fd(raw_fd2) };
    assert_eq!(socket.as_raw_fd(), raw_fd1);
    assert_eq!(socket.local_addr().unwrap(), address);
}

#[test]
fn udp_socket_deregister() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let thread_handle = send_packets(address, 1, barrier.clone());

    os_queue.register(&mut socket, ID1, UdpSocket::INTERESTS, RegisterOption::EDGE).unwrap();
    os_queue.deregister(&mut socket).unwrap();

    // Let the packet be send.
    barrier.wait();

    // Shouldn't get any events after deregistering.
    events.clear();
    poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(500)))
        .expect("unable to poll");
    assert!(events.is_empty());

    // But we do expect a packet to be send.
    let mut buf = [0; 20];
    let (n, _) = socket.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn udp_socket_reregister() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let thread_handle = send_packets(address, 1, barrier.clone());

    os_queue.register(&mut socket, ID1, Interests::READABLE, RegisterOption::LEVEL).unwrap();
    os_queue.reregister(&mut socket, ID2, Interests::READABLE, RegisterOption::LEVEL).unwrap();

    // Let the packet be send.
    barrier.wait();

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(ID2, Ready::READABLE),
    ]);

    let mut buf = [0; 20];
    let (n, _) = socket.recv_from(&mut buf).unwrap();
    assert_eq!(n, DATA1.len());
    assert_eq!(buf[..n], DATA1[..]);

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn udp_socket_edge_poll_option_drain() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let thread_handle = send_packets(address, 2, barrier.clone());

    os_queue.register(&mut socket, ID1, Interests::READABLE, RegisterOption::EDGE)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_events = 0;
    for _ in 0..4  {
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(100))).unwrap();

        for event in events.drain(..) {
            match event.id() {
                ID1 if seen_events == 0 => {
                    let mut buf = [0; 20];
                    assert_eq!(socket.recv_from(&mut buf).unwrap().0, DATA1.len());
                    assert_would_block(socket.recv_from(&mut buf));
                    seen_events = 1;

                    // Unblock second packet.
                    barrier.wait();
                },
                ID1 if seen_events == 1 => {
                    let mut buf = [0; 20];
                    assert_eq!(socket.recv_from(&mut buf).unwrap().0, DATA1.len());
                    assert_would_block(socket.recv_from(&mut buf));
                    seen_events = 2;
                },
                ID1 => panic!("unexpected event for level UDP socket"),
                _ => unreachable!(),
            }
        }
    }
    assert!(seen_events == 2, "didn't see any events");

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn udp_socket_oneshot_poll_option() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let thread_handle = send_packets(address, 2, barrier.clone());

    os_queue.register(&mut socket, ID1, Interests::READABLE, RegisterOption::ONESHOT)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_event = false;
    for _ in 0..2 {
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(100))).unwrap();

        for event in events.drain(..) {
            match event.id() {
                ID1 if !seen_event => {
                    seen_event = true;

                    // Unblock the first packet.
                    barrier.wait();
                },
                ID1 => panic!("unexpected event for oneshot UDP socket"),
                _ => unreachable!(),
            }
        }
    }
    assert!(seen_event, "didn't see any events");

    thread_handle.join().expect("unable to join thread");
}

#[test]
fn udp_socket_oneshot_poll_option_reregister() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket = UdpSocket::bind(any_local_address()).unwrap();
    let address = socket.local_addr().unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let thread_handle = send_packets(address, 2, barrier.clone());

    os_queue.register(&mut socket, ID1, Interests::READABLE, RegisterOption::ONESHOT)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_event = false;
    for _ in 0..2 {
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(100))).unwrap();

        for event in events.drain(..) {
            match event.id() {
                ID1 if !seen_event => seen_event = true,
                ID1 => panic!("unexpected event for oneshot UDP socket"),
                _ => unreachable!(),
            }
        }
    }
    assert!(seen_event, "didn't see any events");

    // Reregister the socket and we expect to see more events.
    os_queue.reregister(&mut socket, ID2, Interests::READABLE, RegisterOption::ONESHOT).unwrap();

    // Unblock the first packet.
    barrier.wait();

    seen_event = false;
    for _ in 0..2 {
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, Some(Duration::from_millis(100))).unwrap();

        for event in events.drain(..) {
            match event.id() {
                ID2 if !seen_event => seen_event = true,
                ID2 => panic!("unexpected event for oneshot UDP socket"),
                _ => unreachable!(),
            }
        }
    }
    assert!(seen_event, "didn't see any events");

    thread_handle.join().expect("unable to join thread");
}

/// Sends `n_packets` packets to `address`, over UDP, after the `barrier` is
/// waited (before each send) on in another thread.
fn send_packets(address: SocketAddr, n_packets: usize, barrier: Arc<Barrier>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let socket = net::UdpSocket::bind(any_local_address()).unwrap();
        for _ in 0..n_packets {
            barrier.wait();
            assert_eq!(socket.send_to(DATA1, address).unwrap(), DATA1.len());
        }
    })
}
