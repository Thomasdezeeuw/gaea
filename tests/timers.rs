use std::time::{Duration, Instant};
use std::thread::sleep;

use log::error;

use mio_st::Timers;
use mio_st::event::{self, Source, Event, Ready};

mod util;

use self::util::init;

const NEXT_EVENT_MARGIN: Duration = Duration::from_millis(1);

#[test]
fn timers() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();
    let id = event::Id(0);

    // No deadlines, no timeout and no events.
    assert_eq!(Source::<Vec<Event>>::next_event_available(&mut timers), None);
    timers.poll(&mut events).unwrap();
    assert!(events.is_empty());

    timers.add_deadline(id, Instant::now());
    // Now we have a deadline which already passed, so no blocking.
    assert_eq!(Source::<Vec<Event>>::next_event_available(&mut timers),
        Some(Duration::from_millis(0)));
    expect_events(&mut timers, &mut events, vec![Event::new(id, Ready::TIMER)]);

    let timeout = Duration::from_millis(50);
    timers.add_timeout(id, timeout);
    // Have a deadline, but it hasn't passed yet.
    roughly_equal(Source::<Vec<Event>>::next_event_available(&mut timers).unwrap(),
        timeout);
    // So no events.
    expect_events(&mut timers, &mut events, vec![]);

    // But after the deadline expires we should have an event.
    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![Event::new(id, Ready::TIMER)]);
}

#[test]
fn timers_multiple_deadlines_same_time() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();

    let deadline = Instant::now();
    for token in 0..3 {
        timers.add_deadline(event::Id(token), deadline);
    }

    expect_events(&mut timers, &mut events, vec![
        Event::new(event::Id(0), Ready::TIMER),
        Event::new(event::Id(1), Ready::TIMER),
        Event::new(event::Id(2), Ready::TIMER),
    ]);
}

#[test]
fn timers_multiple_deadlines_same_id() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();

    let timeout = Duration::from_millis(10);
    timers.add_timeout(event::Id(0), timeout * 2);
    timers.add_timeout(event::Id(0), timeout);

    roughly_equal(Source::<Vec<Event>>::next_event_available(&mut timers).unwrap(),
        timeout);

    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![Event::new(event::Id(0), Ready::TIMER)]);
    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![Event::new(event::Id(0), Ready::TIMER)]);
}

#[test]
fn timers_multiple_deadlines_same_time_andid() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();

    let timeout = Duration::from_millis(10);
    timers.add_timeout(event::Id(0), timeout);
    timers.add_timeout(event::Id(0), timeout);

    roughly_equal(Source::<Vec<Event>>::next_event_available(&mut timers).unwrap(),
        timeout);

    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![
        Event::new(event::Id(0), Ready::TIMER),
        Event::new(event::Id(0), Ready::TIMER),
    ]);
}

#[test]
fn timers_remove_deadline() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();
    let id = event::Id(0);
    let timeout = Duration::from_millis(50);

    timers.add_deadline(id, Instant::now() + timeout);
    expect_events(&mut timers, &mut events, vec![]);

    timers.remove_deadline(id);

    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![]);

    timers.add_timeout(id, timeout);
    expect_events(&mut timers, &mut events, vec![]);

    // This should also remove the timeout.
    timers.remove_deadline(id);

    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![]);
}

/// Assert that `left` and `right` are roughly equal, with a margin of
/// `NEXT_EVENT_MARGIn` difference.
fn roughly_equal(left: Duration, right: Duration) {
    // Add a duration to not underflow.
    const ADD: Duration = Duration::from_secs(100);
    let diff = (ADD + left) - right;
    assert!(diff < (NEXT_EVENT_MARGIN + ADD), "wanted {:?}, but got {:?}", left, right);
}

/// Poll `Timers` for events.
fn expect_events(timers: &mut Timers, events: &mut Vec<Event>, mut expected: Vec<Event>) {
    events.clear();
    timers.poll(events).expect("unable to poll timers");

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
