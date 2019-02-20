use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::{Arc, Barrier};
use std::thread::{self, sleep};
use std::time::Duration;

use mio_st::event::{Event, Ready};
use mio_st::net::{ConnectedUdpSocket, UdpSocket};
use mio_st::os::{PollOption, Interests};
use mio_st::{event, poll};

mod util;

use self::util::{assert_would_block, any_local_address, any_local_ipv6_address, expect_events, init, init_with_os_queue};

const DATA1: &'static [u8; 12] = b"Hello world!";
const DATA2: &'static [u8; 11] = b"Hello mars!";

const ID1: event::Id = event::Id(0);
const ID2: event::Id = event::Id(1);

#[test]
fn udp_socket() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_address()).unwrap();

    let address1 = socket1.local_addr().unwrap();
    let address2 = socket2.local_addr().unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, PollOption::Edge)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, PollOption::Edge)
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
#[cfg_attr(feature="disable_test_ipv6", ignore = "skipping IPv6 test")]
fn udp_socket_ipv6() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_ipv6_address()).unwrap();
    let mut socket2 = UdpSocket::bind(any_local_ipv6_address()).unwrap();

    let address1 = socket1.local_addr().unwrap();
    let address2 = socket2.local_addr().unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, PollOption::Edge)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, PollOption::Edge)
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

    let mut socket2 = ConnectedUdpSocket::connect(any_local_address(), address1).unwrap();
    let address2 = socket2.local_addr().unwrap();

    let mut socket1 = socket1.connect(address2).unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, PollOption::Edge)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, PollOption::Edge)
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
#[cfg_attr(feature="disable_test_ipv6", ignore = "skipping IPv6 test")]
fn connected_udp_socket_ipv6() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let mut socket1 = UdpSocket::bind(any_local_ipv6_address()).unwrap();
    let address1 = socket1.local_addr().unwrap();

    let mut socket2 = ConnectedUdpSocket::connect(any_local_ipv6_address(), address1).unwrap();
    let address2 = socket2.local_addr().unwrap();

    let mut socket1 = socket1.connect(address2).unwrap();

    os_queue.register(&mut socket1, ID1, UdpSocket::INTERESTS, PollOption::Edge)
        .expect("unable to register UDP socket");
    os_queue.register(&mut socket2, ID2, UdpSocket::INTERESTS, PollOption::Edge)
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
fn connected_udp_socket_raw_fd() {
    init();

    let listen_socket = net::UdpSocket::bind(any_local_address()).unwrap();
    let connect_address = listen_socket.local_addr().unwrap();

    let mut socket = ConnectedUdpSocket::connect(any_local_address(), connect_address).unwrap();
    let address = socket.local_addr().unwrap();

    let raw_fd1 = socket.as_raw_fd();
    let raw_fd2 = socket.into_raw_fd();
    assert_eq!(raw_fd1, raw_fd2);

    let mut socket = unsafe { ConnectedUdpSocket::from_raw_fd(raw_fd2) };
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

    os_queue.register(&mut socket, ID1, UdpSocket::INTERESTS, PollOption::Edge).unwrap();
    os_queue.deregister(&mut socket).unwrap();

    // Let the packet be send.
    barrier.wait();

    // Shouldn't get any events after deregistering.
    events.clear();
    poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(500)))
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

    os_queue.register(&mut socket, ID1, Interests::READABLE, PollOption::Level).unwrap();
    os_queue.reregister(&mut socket, ID2, Interests::READABLE, PollOption::Level).unwrap();

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

    os_queue.register(&mut socket, ID1, Interests::READABLE, PollOption::Edge)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_events = 0;
    for _ in 0..4  {
        poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(100))).unwrap();

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

    os_queue.register(&mut socket, ID1, Interests::READABLE, PollOption::Oneshot)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_event = false;
    for _ in 0..2 {
        poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(100))).unwrap();

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

    os_queue.register(&mut socket, ID1, Interests::READABLE, PollOption::Oneshot)
        .expect("unable to register UDP socket");

    // Unblock the first packet.
    barrier.wait();

    let mut seen_event = false;
    for _ in 0..2 {
        poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(100))).unwrap();

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
    os_queue.reregister(&mut socket, ID2, Interests::READABLE, PollOption::Oneshot).unwrap();

    // Unblock the first packet.
    barrier.wait();

    seen_event = false;
    for _ in 0..2 {
        poll(&mut os_queue, &mut [], &mut events, Some(Duration::from_millis(100))).unwrap();

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
