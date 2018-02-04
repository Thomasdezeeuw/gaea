use std::error::Error;
use std::time::{Duration, Instant};

use mio_st::event::{Event, Events, EventedId};
use mio_st::poll::{Poll, Ready};

use {expect_events, init_with_poll};

/// A wrapper function around `expect_events` to check that elapsed time doesn't
/// exceed `max_elapsed`, runs a single poll.
fn expect_events_elapsed(poll: &mut Poll, events: &mut Events, max_elapsed: Duration, expected: Vec<Event>) {
    let start = Instant::now();
    expect_events(poll, events, 1, expected);
    let elapsed = start.elapsed();
    assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
            it returned after: {:?}", max_elapsed, elapsed);
}

/// It's hard to compare the actual deadline, so instead we make it is within a
/// range of time.
fn within_margin(got: Instant, expected: Instant, margin: Duration) {
    assert!(got > (expected - margin), "expected {:?} to be after {:?}", got, (expected - margin));
    assert!(got < (expected + margin), "expected {:?} to be before {:?}", got, (expected + margin));
}

/// Allowed margin for `Poll.poll` to return
const MARGIN_MS: u64 = 10;

#[test]
fn invalid_id() {
    let (mut poll, mut events) = init_with_poll(8);

    let result = poll.add_deadline(EventedId(usize::max_value()), Instant::now());
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("invalid evented id"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn add_deadline() {
    let (mut poll, mut events) = init_with_poll(8);

    poll.add_deadline(EventedId(0), Instant::now()).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn add_timeout() {
    let (mut poll, mut events) = init_with_poll(8);

    poll.add_timeout(EventedId(0), Duration::from_millis(20)).unwrap();
    poll.add_timeout(EventedId(1), Duration::from_millis(10)).unwrap();
    // Same id and timeout should be ok.
    poll.add_timeout(EventedId(0), Duration::from_millis(20)).unwrap();

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(1), Ready::TIMER),
    ]);
    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10 + MARGIN_MS), vec![
        Event::new(EventedId(0), Ready::TIMER),
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn remove_deadline() {
    let (mut poll, mut events) = init_with_poll(8);

    assert!(poll.remove_deadline(EventedId(0)).is_none());

    poll.add_timeout(EventedId(0), Duration::from_millis(10)).unwrap();
    let deadline = poll.remove_deadline(EventedId(0)).unwrap();
    within_margin(deadline, Instant::now() + Duration::from_millis(10), Duration::from_millis(5));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn multiple_deadlines() {
    let (mut poll, mut events) = init_with_poll(64);

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
        if timeout == T1 {
            EventedId(1)
        } else if timeout == T2 {
            EventedId(2)
        } else if timeout == T3 {
            EventedId(3)
        } else {
            unreachable!()
        }
    }

    for timers in &TIMEOUTS {
        for timeout_ms in timers.iter() {
            let token = timeout_to_token(*timeout_ms);
            let first_token = EventedId(token.0 * 100);

            let timeout = Duration::from_millis(*timeout_ms);
            poll.add_timeout(first_token, timeout).unwrap();

            let got_deadline = poll.remove_deadline(first_token).unwrap();
            within_margin(got_deadline, Instant::now() + timeout, Duration::from_millis(5));

            poll.add_timeout(token, timeout).unwrap();
        }

        for token in 1..4 {
            expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MAX_ELAPSED), vec![
                Event::new(EventedId(token), Ready::TIMER),
            ]);
        }
    }
}

#[test]
fn multiple_deadlines_same_deadline() {
    let (mut poll, mut events) = init_with_poll(8);

    let deadline = Instant::now() + Duration::from_millis(10);
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
fn poll_timeout() {
    let (mut poll, mut events) = init_with_poll(8);

    let timeouts = [
        // Should not block.
        (Duration::from_millis(0), Duration::from_millis(MARGIN_MS)),
        // Should set timeout to 20 milliseconds.
        (Duration::from_millis(20), Duration::from_millis(20 + MARGIN_MS)),
        // Should keep the original 50 milliseconds timeout.
        (Duration::from_millis(100), Duration::from_millis(50 + MARGIN_MS)),
    ];

    for &(timeout, max_elapsed) in &timeouts {
        poll.add_timeout(EventedId(0), timeout).unwrap();

        let start = Instant::now();
        poll.poll(&mut events, Some(Duration::from_millis(50))).unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
                it returned after: {:?}", max_elapsed, elapsed);
    }
}
