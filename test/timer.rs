use std::time::{Duration, Instant};

use mio::event::{Event, Events, EventedId};
use mio::poll::{Poll, Ready};

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

#[test]
fn add_deadline() {
    let (mut poll, mut events) = init_with_poll(8);

    poll.add_deadline(EventedId(0), Instant::now());

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn add_timeout() {
    let (mut poll, mut events) = init_with_poll(8);

    poll.add_timeout(EventedId(0), Duration::from_millis(20));
    poll.add_timeout(EventedId(1), Duration::from_millis(10));

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(20), vec![
        Event::new(EventedId(1), Ready::TIMER),
    ]);
    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(20), vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn remove_deadline() {
    let (mut poll, mut events) = init_with_poll(8);

    poll.add_timeout(EventedId(0), Duration::from_millis(10));
    // It's hard to compare the actual deadline returned, so we just make an
    // estimate.
    let deadline = poll.remove_deadline(EventedId(0)).unwrap();
    assert!(deadline > Instant::now() + Duration::from_millis(5));
    assert!(deadline < Instant::now() + Duration::from_millis(10));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn multiple_deadlines() {
    let (mut poll, mut events) = init_with_poll(64);

    const T1: u64 = 20;
    const T2: u64 = T1 * 2;
    const T3: u64 = T1 * 3;
    const MAX_ELAPSED: u64 = T1 + 10;

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

    for timers in TIMEOUTS.iter() {
        for timeout_ms in timers.iter() {
            let token = timeout_to_token(*timeout_ms);
            let first_token = EventedId(token.0 * 100);

            let timeout = Duration::from_millis(*timeout_ms);
            poll.add_timeout(first_token, timeout);
            poll.remove_deadline(first_token);
            poll.add_timeout(token, timeout);
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
        poll.add_deadline(EventedId(token), deadline);
    }

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(20), vec![
        Event::new(EventedId(0), Ready::TIMER),
        Event::new(EventedId(1), Ready::TIMER),
        Event::new(EventedId(2), Ready::TIMER),
    ]);
}

#[test]
fn test_poll_time() {
    let (mut poll, mut events) = init_with_poll(8);

    let deadlines = [
        // Should not block.
        (Instant::now(), Duration::from_millis(5)),
        // Should set timeout to 20 milliseconds.
        (Instant::now() + Duration::from_millis(20), Duration::from_millis(30)),
        // Should keep the original 50 milliseconds timeout.
        (Instant::now() + Duration::from_millis(100), Duration::from_millis(60)),
    ];

    for &(deadline, max_elapsed) in &deadlines {
        poll.add_deadline(EventedId(0), deadline);

        let start = Instant::now();
        poll.poll(&mut events, Some(Duration::from_millis(50))).unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
                it returned after: {:?}", max_elapsed, elapsed);
    }
}
