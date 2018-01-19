use std::time::{Duration, Instant};

use mio::*;
use mio::timer::Timer;

use expect_events;

// TODO: add tests with other Evented items, using different subsystems.
// Poll:
// - prepare_time extension:
//   - Deadline already elapsed -> elapsed as short as possible.
//   - Deadline first before timeout.
//   - Deadline after provided timeout.

#[test]
pub fn register_timer() {
    let _ = ::env_logger::init();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut timer = Timer::timeout(Duration::from_millis(10));
    poll.register(&mut timer, Token(0), Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(20), vec![
        Event::new(Ready::TIMEOUT, Token(0)),
    ]);
}

#[test]
pub fn deregister_timer() {
    let _ = ::env_logger::init();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut timer = Timer::timeout(Duration::from_millis(10));
    poll.register(&mut timer, Token(0), Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
    poll.deregister(&mut timer).unwrap();
    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
pub fn reregister_timer() {
    let _ = ::env_logger::init();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut timer = Timer::timeout(Duration::from_millis(30));
    poll.register(&mut timer, Token(0), Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
    poll.deregister(&mut timer).unwrap();
    poll.reregister(&mut timer, Token(1), Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(40), vec![
        Event::new(Ready::TIMEOUT, Token(1)),
    ]);
}

#[test]
pub fn multiple_timers() {
    let _ = ::env_logger::init();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

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

    fn timeout_to_token(timeout: u64) -> Token {
        if timeout == T1 {
            Token(1)
        } else if timeout == T2 {
            Token(2)
        } else if timeout == T3 {
            Token(3)
        } else {
            unreachable!()
        }
    }

    for timers in TIMEOUTS.iter() {
        for timeout_ms in timers.iter() {
            let token = timeout_to_token(*timeout_ms);
            let first_token = Token(token.0 * 100);

            let mut timer = Timer::timeout(Duration::from_millis(*timeout_ms));
            poll.register(&mut timer, first_token, Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
            poll.deregister(&mut timer).unwrap();
            poll.reregister(&mut timer, token, Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
        }

        for token in 1..4 {
            expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MAX_ELAPSED), vec![
                Event::new(Ready::TIMEOUT, Token(token)),
            ]);
        }
    }
}

#[test]
pub fn multiple_timers_same_deadline() {
    let _ = ::env_logger::init();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let deadline = Instant::now() + Duration::from_millis(20);
    for token in 0..3 {
        let mut timer = Timer::new(deadline);
        poll.register(&mut timer, Token(token), Ready::TIMEOUT, PollOpt::ONESHOT).unwrap();
    }

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(30), vec![
        Event::new(Ready::TIMEOUT, Token(0)),
        Event::new(Ready::TIMEOUT, Token(1)),
        Event::new(Ready::TIMEOUT, Token(2)),
    ]);
}

/// A wrapper function around `expect_events` to check that elapsed time doesn't
/// exceed `max_elapsed`, runs a single poll.
fn expect_events_elapsed(poll: &mut Poll, events: &mut Events, max_elapsed: Duration, expected: Vec<Event>) {
    let start = Instant::now();
    expect_events(poll, events, 1, expected);
    let elapsed = start.elapsed();
    assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
            it returned after: {:?}", max_elapsed, elapsed);
}
