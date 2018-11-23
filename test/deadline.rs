use std::error::Error;
use std::time::{Duration, Instant};

use mio_st::event::{Event, EventedId, Events, Ready};
use mio_st::poll::Poller;

use crate::{expect_events, init_with_poll};

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
const MARGIN_MS: u64 = 10;

#[test]
fn add_deadline() {
    let (mut poll, mut events) = init_with_poll();

    poll.add_deadline(EventedId(0), Instant::now()).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
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
            let duration = Instant::now() + Duration::from_millis(*timeout_ms);

            poll.add_deadline(first_token, duration).unwrap();
            poll.remove_deadline(first_token).unwrap();
            poll.add_deadline(token, duration).unwrap();
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

    let deadline = Instant::now();
    for token in 0..3 {
        poll.add_deadline(EventedId(token), deadline).unwrap();
    }

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
        Event::new(EventedId(1), Ready::TIMER),
        Event::new(EventedId(2), Ready::TIMER),
    ]);
}

#[test]
fn multiple_deadline_same_id() {
    let (mut poll, mut events) = init_with_poll();

    poll.add_deadline(EventedId(0), Instant::now() + Duration::from_millis(20)).unwrap();
    poll.add_deadline(EventedId(0), Instant::now() + Duration::from_millis(10)).unwrap();

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
        poll.add_deadline(EventedId(0), Instant::now() + timeout).unwrap();

        let start = Instant::now();
        poll.poll(&mut events, Some(Duration::from_millis(50))).unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
                it returned after: {:?}", max_elapsed, elapsed);
    }
}
