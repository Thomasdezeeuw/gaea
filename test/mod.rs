extern crate mio_st;

#[macro_use]
extern crate log;
extern crate env_logger;

use std::time::Duration;

use mio_st::poll::Poll;
use mio_st::event::{Event, Events};

/// Initializate the test setup.
pub fn init() {
    let env = env_logger::Env::new().filter("LOG_LEVEL");
    // Logger could already be set, so we ignore the result.
    drop(env_logger::try_init_from_env(env));
}

/// Initializate the test setup (same as init) and create a Poll instance and
/// events.
pub fn init_with_poll(events_capacity: usize) -> (Poll, Events) {
    init();
    let poll = Poll::new().expect("unable to create Poll instance");
    let events = Events::with_capacity(events_capacity, events_capacity);
    (poll, events)
}

/// Poll `poll` and compare the retrieved events with the `expected` ones.
pub fn expect_events(poll: &mut Poll, events: &mut Events, poll_try_count: usize, mut expected: Vec<Event>) {
    let timeout = Duration::from_millis(1_000);

    for _ in 0..poll_try_count {
        debug!(target: "expect_events", "polling");
        poll.poll(events, Some(timeout)).expect("unable to poll");

        for event in &mut *events {
            debug!(target: "expect_events", "got event: {:?}", event);

            let pos = expected.iter()
                .position(|exp_event| {
                    event.id() == exp_event.id() &&
                    event.readiness().contains(exp_event.readiness())
                });

            if let Some(pos) = pos {
                debug!(target: "expect_events", "got an event match");
                drop(expected.remove(pos));
            }
        }

        if expected.is_empty() {
            break;
        }
    }

    assert!(expected.is_empty(), "The following expected events were not found: {:?}", expected);
}

mod event;
mod registering;
mod timer;
mod userspace_registration;
