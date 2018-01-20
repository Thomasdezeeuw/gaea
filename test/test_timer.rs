use std::time::{Duration, Instant};

use mio::event::{Event, Events};
use mio::poll::{Poll, Ready, Token};

use expect_events;

// TODO: add tests with other Evented items, using different subsystems.
// Poll:
// - prepare_time extension:
//   - Deadline already elapsed -> elapsed as short as possible.
//   - Deadline first before timeout.
//   - Deadline after provided timeout.

fn setup() -> (Poll, Events) {
    let _ = ::env_logger::init();
    let poll = Poll::new().unwrap();
    let events = Events::with_capacity(16);
    (poll, events)
}

#[test]
pub fn add_deadline() {
    let (mut poll, mut events) = setup();

    poll.add_deadline(Token(0), Instant::now());

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(10), vec![
        Event::new(Token(0), Ready::TIMER),
    ]);
}

#[test]
pub fn add_timeout() {
    let (mut poll, mut events) = setup();

    poll.add_timeout(Token(0), Duration::from_millis(10));

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(20), vec![
        Event::new(Token(0), Ready::TIMER),
    ]);
}

#[test]
pub fn remove_deadline() {
    let (mut poll, mut events) = setup();

    poll.add_timeout(Token(0), Duration::from_millis(10));
    poll.remove_deadline(Token(0));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
pub fn multiple_deadlines() {
    let (mut poll, mut events) = setup();

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

            let timeout = Duration::from_millis(*timeout_ms);
            poll.add_timeout(first_token, timeout);
            poll.remove_deadline(first_token);
            poll.add_timeout(token, timeout);
        }

        for token in 1..4 {
            expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(MAX_ELAPSED), vec![
                Event::new(Token(token), Ready::TIMER),
            ]);
        }
    }
}

#[test]
pub fn multiple_deadlines_same_deadline() {
    let (mut poll, mut events) = setup();

    let deadline = Instant::now() + Duration::from_millis(20);
    for token in 0..3 {
        poll.add_deadline(Token(token), deadline);
    }

    expect_events_elapsed(&mut poll, &mut events, Duration::from_millis(30), vec![
        Event::new(Token(0), Ready::TIMER),
        Event::new(Token(1), Ready::TIMER),
        Event::new(Token(2), Ready::TIMER),
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
