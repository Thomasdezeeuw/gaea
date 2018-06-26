extern crate mio_st;

extern crate env_logger;
#[macro_use]
extern crate log;

use std::net::SocketAddr;
use std::time::Duration;

use mio_st::poll::Poller;
use mio_st::event::{Event, Events};

/// Initializate the test setup.
pub fn init() {
    let env = env_logger::Env::new().filter("LOG_LEVEL");
    // Logger could already be set, so we ignore the result.
    drop(env_logger::try_init_from_env(env));
}

/// Initializate the test setup (same as init) and create a Poller instance and
/// events.
pub fn init_with_poll() -> (Poller, Events) {
    init();
    let poll = Poller::new().expect("unable to create Poller instance");
    let events = Events::new();
    (poll, events)
}

/// Poller `poll` and compare the retrieved events with the `expected` ones.
pub fn expect_events(poll: &mut Poller, events: &mut Events, poll_try_count: usize, mut expected: Vec<Event>) {
    let timeout = Duration::from_millis(1_000);

    for _ in 0..poll_try_count {
        debug!(target: "expect_events", "polling");
        poll.poll(events, Some(timeout)).expect("unable to poll");

        debug!(target: "expect_events", "got {} events", events.len());
        for event in &mut *events {
            debug!(target: "expect_events", "got event: {:?}", event);

            let pos = expected.iter()
                .position(|exp_event| {
                    event.id() == exp_event.id() &&
                    event.readiness().contains(exp_event.readiness())
                });

            if let Some(pos) = pos {
                debug!(target: "expect_events", "got an event match");
                expected.remove(pos);
            }
        }

        if expected.is_empty() {
            break;
        }
    }

    assert!(expected.is_empty(), "The following expected events were not found: {:?}", expected);
}

/// Bind to any port on localhost.
pub fn any_port() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

mod event;
mod poll;
mod registering;
mod tcp;
mod timer;
mod udp;
mod userspace_registration;
