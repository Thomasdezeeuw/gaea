use std::error::Error;
use std::time::{Duration, Instant};

use mio_st::event::{Event, EventedId, Events, Ready};
use mio_st::poll::{Poller, PollOption};
use mio_st::timer::Timer;

use {expect_events, init_with_poll};

/// A wrapper function around `expect_events` to check that elapsed time doesn't
/// exceed `max_elapsed`, runs a single poll.
fn expect_events_elapsed(poll: &mut Poller, events: &mut Events, max_elapsed: Duration, expected: Vec<Event>) {
    let start = Instant::now();
    expect_events(poll, events, 1, expected);
    let elapsed = start.elapsed();
    assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
            it returned after: {:?}", max_elapsed, elapsed);
}

/// Allowed margin for `Poller.poll` to return
#[cfg(not(feature = "test_extended_time_margin"))]
const MARGIN_MS: u64 = 10;
#[cfg(feature = "test_extended_time_margin")]
const MARGIN_MS: u64 = 50;

// TODO: test panics using incorrect opt/interests. Deregister without
// registering first.

#[test]
fn invalid_id() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer = Timer::deadline(Instant::now());
    let result = poll.register(&mut timer, EventedId(usize::max_value()), Ready::TIMER, PollOption::Oneshot);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("invalid evented id"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn deadline() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer = Timer::deadline(Instant::now());
    poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn timeout() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer = Timer::timeout(Duration::from_millis(10));
    poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn deregistering() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer = Timer::deadline(Instant::now());
    poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();
    poll.deregister(&mut timer).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
#[should_panic(expected = "trying to (re)register `Timer` with interests other then `TIMER`")]
fn incorrect_readiness() {
    let (mut poll, _) = init_with_poll();
    let mut timer = Timer::deadline(Instant::now());
    poll.register(&mut timer, EventedId(0), Ready::READABLE, PollOption::Oneshot).unwrap();
}

#[test]
#[should_panic(expected = "trying to (re)register `Timer` with poll option other then `Oneshot`")]
fn incorrect_poll_option() {
    let (mut poll, _) = init_with_poll();
    let mut timer = Timer::deadline(Instant::now());
    poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Level).unwrap();
}

#[test]
fn multiple_timers() {
    let (mut poll, mut events) = init_with_poll();

    const T1: u64 = 20;
    const T2: u64 = T1 * 2;
    const T3: u64 = T1 * 3;
    const MAX_ELAPSED: u64 = T1 + MARGIN_MS;

    const TIMEOUTS: [[u64; 3]; 6] = [
        [T1, T2, T3],
        [T1, T3, T2],
        [T2, T1, T3],
        [T2, T3, T1],
        [T3, T1, T2],
        [T3, T2, T1],
    ];

    fn timeout_to_token(timeout: u64) -> EventedId {
        match timeout {
            T1 => EventedId(1),
            T2 => EventedId(2),
            T3 => EventedId(3),
            _ => unreachable!()
        }
    }

    for timeouts in &TIMEOUTS {
        for timeout_ms in timeouts.iter() {
            let token = timeout_to_token(*timeout_ms);
            let first_token = EventedId(token.0 * 100);

            let mut timer = Timer::timeout(Duration::from_millis(*timeout_ms));

            poll.register(&mut timer, first_token, Ready::TIMER, PollOption::Oneshot).unwrap();
            poll.deregister(&mut timer).unwrap();

            poll.register(&mut timer, token, Ready::TIMER, PollOption::Oneshot).unwrap();
        }

        for token in 1..4 {
            expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MAX_ELAPSED), vec![
                Event::new(EventedId(token), Ready::TIMER),
            ]);
        }
    }
}

#[test]
fn multiple_timers_same_deadline() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer = Timer::deadline(Instant::now());
    for token in 0..3 {
        poll.register(&mut timer, EventedId(token), Ready::TIMER, PollOption::Oneshot).unwrap();
    }

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
        Event::new(EventedId(1), Ready::TIMER),
        Event::new(EventedId(2), Ready::TIMER),
    ]);
}

#[test]
fn multiple_timers_same_id() {
    let (mut poll, mut events) = init_with_poll();

    let mut timer1 = Timer::timeout(Duration::from_millis(20));
    poll.register(&mut timer1, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();

    let mut timer2 = Timer::timeout(Duration::from_millis(10));
    poll.register(&mut timer2, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn poll_should_set_timeout() {
    let (mut poll, mut events) = init_with_poll();

    let timeouts = [
        // Should not block.
        (Duration::from_millis(0), Duration::from_millis(MARGIN_MS)),
        // Should set timeout to 20 milliseconds.
        (Duration::from_millis(20), Duration::from_millis(20 + MARGIN_MS)),
        // Should keep the original 50 milliseconds timeout.
        (Duration::from_millis(100), Duration::from_millis(50 + MARGIN_MS)),
    ];

    for &(timeout, max_elapsed) in &timeouts {
        let mut timer = Timer::timeout(timeout);
        poll.register(&mut timer, EventedId(0), Ready::TIMER, PollOption::Oneshot).unwrap();

        let start = Instant::now();
        poll.poll(&mut events, Some(Duration::from_millis(50))).unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
                it returned after: {:?}", max_elapsed, elapsed);
    }
}
