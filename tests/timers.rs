use std::thread::sleep;
use std::time::{Duration, Instant};

use mio_st::event::{self, Capacity, Event, Ready, Source};
use mio_st::Timers;

mod util;

use self::util::{init, next_event_available, expect_events, EventsCapacity};

#[test]
fn timers() {
    init();
    let mut timers = Timers::new();
    let mut events = Vec::new();
    let id = event::Id(0);

    // No deadlines, no timeout and no events.
    assert_eq!(next_event_available(&mut timers), None);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert!(events.is_empty());

    timers.add_deadline(id, Instant::now());
    // Now we have a deadline which already passed, so no blocking.
    assert_eq!(next_event_available(&mut timers), Some(Duration::from_millis(0)));
    expect_events(&mut timers, &mut events, vec![Event::new(id, Ready::TIMER)]);

    let timeout = Duration::from_millis(50);
    timers.add_timeout(id, timeout);
    // Have a deadline, but it hasn't passed yet.
    roughly_equal(next_event_available(&mut timers).unwrap(), timeout);
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
    timers.add_timeout(event::Id(0), timeout * 10);
    timers.add_timeout(event::Id(0), timeout);

    roughly_equal(next_event_available(&mut timers).unwrap(), timeout);

    sleep(timeout);
    expect_events(&mut timers, &mut events, vec![Event::new(event::Id(0), Ready::TIMER)]);
    sleep(timeout * 9);
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

    roughly_equal(next_event_available(&mut timers).unwrap(), timeout);

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

#[test]
fn timers_events_capacity() {
    init();
    let mut timers = Timers::new();

    let id = event::Id(0);
    let deadline = Instant::now();
    timers.add_deadline(id, deadline);
    timers.add_deadline(id, deadline);

    let mut events = EventsCapacity(Capacity::Limited(0), 0);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert_eq!(events.1, 0); // Shouldn't have grow.

    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert_eq!(events.1, 1);

    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert_eq!(events.1, 1);

    let mut events = EventsCapacity(Capacity::Limited(100), 0);
    timers.add_deadline(id, deadline);
    timers.add_deadline(id, deadline);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert_eq!(events.1, 2);

    let mut events = EventsCapacity(Capacity::Growable, 0);
    timers.add_deadline(id, deadline);
    timers.add_deadline(id, deadline);
    Source::<_, ()>::poll(&mut timers, &mut events).unwrap();
    assert_eq!(events.1, 2);
}

/// Assert that `left` and `right` are roughly equal, with a margin of
/// `DURATION_MARGIN` difference.
fn roughly_equal(left: Duration, right: Duration) {
    const DURATION_MARGIN: Duration = Duration::from_millis(1);
    // Add a duration to not underflow.
    const ADD: Duration = Duration::from_secs(100);
    let diff = (ADD + left) - right;
    assert!(diff < (DURATION_MARGIN + ADD), "wanted {:?}, but got {:?}", left, right);
}
