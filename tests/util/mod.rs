//! Collection of testing utilities.

// Not all functions are used in all tests, causing warnings of unused functions
// while other tests are actually using them.
#![allow(dead_code)]

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use log::error;

use mio_st::event::Event;
use mio_st::poll::Poller;

/// Initialise the test setup, things like logging etc.
pub fn init() {
    let env = env_logger::Env::new().filter("LOG_LEVEL");
    // Logger could already be set, so we ignore the result.
    drop(env_logger::try_init_from_env(env));
}

/// Initialise the test setup (same as init) and create a `Poller` instance and
/// `Events` at the same time.
pub fn init_with_poller() -> (Poller, Vec<Event>) {
    init();

    let poller = Poller::new().expect("unable to create Poller instance");
    let events = Vec::new();
    (poller, events)
}

/// Poll the user space events of `poller` and test if all and only the
/// `expected` events are present. This is strict test and fails if any events
/// are missing or if more events are returned.
pub fn expect_userspace_events(poller: &mut Poller, events: &mut Vec<Event>, mut expected: Vec<Event>) {
    events.clear();
    poller.poll_userspace(events);

    for event in events.drain(..) {
        let index = expected.iter().position(|expected| *expected == event);
        if let Some(index) = index {
            assert_eq!(event, expected.swap_remove(index));
        } else {
            panic!("got unexpected user space event: {:?}", event);
        }
    }

    assert!((&*events).is_empty(), "got extra user space events: {:?}", events);
    assert!(expected.is_empty(), "didn't get all expected user space events, missing: {:?}", expected);
}

/// Poll `poller` and compare the retrieved events with the `expected` ones.
/// This test is looser then `expect_userspace_events`, it only check if an
/// events readiness contains the expected readiness and the ids match.
pub fn expect_events(poller: &mut Poller, events: &mut Vec<Event>, mut expected: Vec<Event>) {
    events.clear();
    poller.poll(events, Some(Duration::from_millis(500)))
        .expect("unable to poll");

    for event in events.drain(..) {
        let index = expected.iter()
            .position(|expected| {
                event.id() == expected.id() &&
                event.readiness().contains(expected.readiness())
            });

        if let Some(index) = index {
            expected.swap_remove(index);
        } else {
            // Must accept sporadic events.
            error!("got unexpected event: {:?}", event);
        }
    }

    assert!(expected.is_empty(), "the following expected events were not found: {:?}", expected);
}

/// Assert that `result` is an error and the formatted error (via
/// `fmt::Display`) equals `expected_msg`.
pub fn assert_error<T, E: ToString>(result: Result<T, E>, expected_msg: &str) {
    match result {
        Ok(_) => panic!("unexpected OK result"),
        Err(err) => assert_eq!(err.to_string(), expected_msg),
    }
}

/// Assert that the provided result is an `io::Error` with kind `WouldBlock`.
pub fn assert_would_block<T>(result: io::Result<T>) {
    match result {
        Ok(_) => panic!("unexpected OK result, expected a `WouldBlock` error"),
        Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {},
        Err(err) => panic!("unexpected error result: {}", err),
    }
}

/// Bind to any port on localhost.
pub fn any_local_address() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Bind to any port on localhost, using a IPv6 address.
pub fn any_local_ipv6_address() -> SocketAddr {
    "[::1]:0".parse().unwrap()
}
