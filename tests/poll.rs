use std::io;
use std::iter::repeat;
use std::thread::sleep;
use std::time::{Duration, Instant};

use mio_st::event::{Event, EventedId, Events, Evented, Ready};
use mio_st::poll::{Poller, PollOption, PollCalled};

mod util;

use self::util::{assert_error, expect_events, expect_userspace_events, init, init_with_poller};

/// Total capacity of `Events`.
const EVENTS_CAP: usize = 256;

/// Allowed margin for `Poller.poll` to overrun the deadline.
const MARGIN: Duration = Duration::from_millis(10);

struct TestEvented {
    registrations: Vec<(EventedId, Ready, PollOption)>,
    reregistrations: Vec<(EventedId, Ready, PollOption)>,
    deregister_count: usize,
}

impl TestEvented {
    fn new() -> TestEvented {
        TestEvented {
            registrations: Vec::new(),
            reregistrations: Vec::new(),
            deregister_count: 0,
        }
    }
}

impl Evented for TestEvented {
    fn register(&mut self, _poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, _p: PollCalled) -> io::Result<()> {
        self.registrations.push((id, interests, opt));
        Ok(())
    }

    fn reregister(&mut self, _poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, _p: PollCalled) -> io::Result<()> {
        self.reregistrations.push((id, interests, opt));
        Ok(())
    }

    fn deregister(&mut self, _poll: &mut Poller, _p: PollCalled) -> io::Result<()> {
        self.deregister_count += 1;
        Ok(())
    }
}

#[test]
fn poller_registration() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = TestEvented::new();
    let id = EventedId(0);
    let interests = Ready::READABLE;
    let opt = PollOption::Edge;
    poller.register(&mut handle, id, interests, opt)
        .expect("unable to register evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.registrations.get(0), Some(&(id, interests, opt)));
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);

    let re_id = EventedId(0);
    let re_interests = Ready::READABLE;
    let re_opt = PollOption::Edge;
    poller.reregister(&mut handle, re_id, re_interests, re_opt)
        .expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.reregistrations.get(0), Some(&(re_id, re_interests, re_opt)));
    assert_eq!(handle.deregister_count, 0);

    poller.deregister(&mut handle).expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.deregister_count, 1);
}

struct ErroneousTestEvented;

impl Evented for ErroneousTestEvented {
    fn register(&mut self, _poll: &mut Poller, _id: EventedId, _interests: Ready, _opt: PollOption, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "register"))
    }

    fn reregister(&mut self, _poll: &mut Poller, _id: EventedId, _interests: Ready, _opt: PollOption, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "reregister"))
    }

    fn deregister(&mut self, _poll: &mut Poller, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "deregister"))
    }
}

#[test]
fn poller_erroneous_registration() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = ErroneousTestEvented;
    let id = EventedId(0);
    let interests = Ready::READABLE;
    let opt = PollOption::Edge;
    assert_error(poller.register(&mut handle, id, interests, opt), "register");
    assert_error(poller.reregister(&mut handle, id, interests, opt), "reregister");
    assert_error(poller.deregister(&mut handle), "deregister");
}

#[test]
fn poller_register_empty_interests() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = TestEvented::new();
    let id = EventedId(0);
    let interests = Ready::empty();
    let opt = PollOption::Edge;
    assert_error(poller.register(&mut handle, id, interests, opt),
                 "registering with empty interests");

    assert_error(poller.reregister(&mut handle, id, interests, opt),
                 "registering with empty interests");

    assert!(handle.registrations.is_empty());
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);
}

#[test]
fn poller_notify() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    let ready = Ready::READABLE;
    poller.notify(id, ready).expect("unable to notify");
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, ready)]);
}

#[test]
fn poller_notify_empty_interests() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let id = EventedId(0);
    let ready = Ready::empty();
    assert_error(poller.notify(id, ready), "registering with empty interests");
}

#[test]
fn poller_add_deadline() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    poller.add_deadline(id, Instant::now());
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, Ready::TIMER)]);

    let timeout = Duration::from_millis(50);
    poller.add_deadline(id, Instant::now() + timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);

    sleep(timeout);
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, Ready::TIMER)]);
}

#[test]
fn poller_remove_deadline() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    let timeout = Duration::from_millis(50);
    poller.add_deadline(id, Instant::now() + timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);

    poller.remove_deadline(id);

    sleep(timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);
}

#[test]
fn poller_add_multiple_deadlines() {
    let (mut poller, mut events) = init_with_poller();

    const T1: Duration = Duration::from_millis(20);
    const T2: Duration = Duration::from_millis(40);
    const T3: Duration = Duration::from_millis(60);

    const TIMEOUTS: [[Duration; 3]; 6] = [
        [T1, T2, T3],
        [T1, T3, T2],
        [T2, T1, T3],
        [T2, T3, T1],
        [T3, T1, T2],
        [T3, T2, T1],
    ];

    fn timeout_to_token(timeout: Duration) -> EventedId {
        match timeout {
            T1 => EventedId(1),
            T2 => EventedId(2),
            T3 => EventedId(3),
            _ => unreachable!()
        }
    }

    for timeouts in &TIMEOUTS {
        for timeout in timeouts.iter() {
            let token = timeout_to_token(*timeout);
            let first_token = EventedId(token.0 * 100);
            let duration = Instant::now() + *timeout;

            poller.add_deadline(first_token, duration);
            poller.remove_deadline(first_token);
            poller.add_deadline(token, duration);
        }

        for token in 1..4 {
            expect_events_elapsed(&mut poller, &mut events, T1 + MARGIN, vec![
                Event::new(EventedId(token), Ready::TIMER),
            ]);
        }
    }
}

#[test]
fn poller_add_multiple_deadlines_same_time() {
    let (mut poller, mut events) = init_with_poller();

    let deadline = Instant::now();
    for token in 0..3 {
        poller.add_deadline(EventedId(token), deadline);
    }

    expect_events_elapsed(&mut poller, &mut events, MARGIN, vec![
        Event::new(EventedId(0), Ready::TIMER),
        Event::new(EventedId(1), Ready::TIMER),
        Event::new(EventedId(2), Ready::TIMER),
    ]);
}

#[test]
fn poller_add_multiple_deadlines_same_id() {
    let (mut poller, mut events) = init_with_poller();

    poller.add_deadline(EventedId(0), Instant::now() + Duration::from_millis(20));
    poller.add_deadline(EventedId(0), Instant::now() + Duration::from_millis(10));

    expect_events_elapsed(&mut poller, &mut events, Duration::from_millis(10) + MARGIN, vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
    expect_events_elapsed(&mut poller, &mut events, Duration::from_millis(10) + MARGIN, vec![
        Event::new(EventedId(0), Ready::TIMER),
    ]);
}

#[test]
fn poller_deterime_timeout() {
    let (mut poller, mut events) = init_with_poller();

    const POLL_TIMEOUT: Duration = Duration::from_millis(50);

    let timeouts = [
        // Has user space events, so no blocking.
        (None, true, Duration::from_millis(0)),
        // Has an expired deadline, so no blocking.
        (Some(Duration::from_millis(0)), false, Duration::from_millis(0)),
        // Next deadline shorter then timeout, so use deadline as timeout.
        (Some(Duration::from_millis(20)), false, Duration::from_millis(20)),
        // Next deadline longer then timeout, so use provided timeout.
        (Some(Duration::from_millis(100)), false, POLL_TIMEOUT),
        // No user space or deadlines, so use provided timeout.
        (None, false, POLL_TIMEOUT),
    ];

    for &(timeout, add_event, max_elapsed) in &timeouts {
        if let Some(timeout) = timeout {
            poller.add_deadline(EventedId(0), Instant::now() + timeout);
        }

        if add_event {
            poller.notify(EventedId(1), Ready::READABLE)
                .expect("can't add user space event");
        }

        let start = Instant::now();
        poller.poll(&mut events, Some(POLL_TIMEOUT)).expect("unable to poll");
        let elapsed = start.elapsed();
        assert!(elapsed < max_elapsed + MARGIN, "expected poll to return within {:?}, \
            it returned after: {:?}", max_elapsed, elapsed);
    }
}

#[test]
fn poller_poll_userspace_internal() {
    let (mut poller, mut events) = init_with_poller();

    // No events.
    expect_events(&mut poller, &mut events, Vec::new());

    // Single event, fits in `Events`.
    poller.notify(EventedId(0), Ready::READABLE)
        .expect("unable to add user space event");
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::READABLE)
    ]);

    // Maximum capacity events, all fit in `Events`.
    let expected: Vec<Event> = repeat(Event::new(EventedId(0), Ready::READABLE))
        .take(EVENTS_CAP)
        .map(|event| {
            poller.notify(event.id(), event.readiness())
                .expect("unable to add user space event");
            event
        })
        .collect();
    expect_events(&mut poller, &mut events, expected);
    expect_events(&mut poller, &mut events, Vec::new());

    // More events then `Events` can handle, the remaining events should be
    // returned in the next call to poll.
    let mut expected: Vec<Event> = repeat(Event::new(EventedId(0), Ready::READABLE))
        .take(EVENTS_CAP + 1)
        .map(|event| {
            poller.notify(event.id(), event.readiness())
                .expect("unable to add user space event");
            event
        })
        .collect();
    let last = vec![expected.pop().unwrap()];
    expect_events(&mut poller, &mut events, expected);
    expect_events(&mut poller, &mut events, last);
}

#[test]
fn poller_poll_deadlines_internal() {
    let (mut poller, mut events) = init_with_poller();
    let now = Instant::now();

    // No deadlines.
    expect_events(&mut poller, &mut events, Vec::new());

    // Single deadline, fits in `Events`.
    poller.add_deadline(EventedId(0), now);
    expect_events(&mut poller, &mut events, vec![
        Event::new(EventedId(0), Ready::TIMER)
    ]);

    // Maximum capacity deadlines, all fit in `Events`.
    let expected: Vec<Event> = repeat(Event::new(EventedId(0), Ready::TIMER))
        .take(EVENTS_CAP)
        .map(|event| { poller.add_deadline(event.id(), now); event})
        .collect();
    expect_events(&mut poller, &mut events, expected);
    expect_events(&mut poller, &mut events, Vec::new());

    // More deadlines then `Events` can handle, the remaining deadlines should
    // be returned in the next call to poll.
    let mut expected: Vec<Event> = repeat(Event::new(EventedId(0), Ready::TIMER))
        .take(EVENTS_CAP + 1)
        .map(|event| { poller.add_deadline(event.id(), now); event})
        .collect();
    let last = vec![expected.pop().unwrap()];
    expect_events(&mut poller, &mut events, expected);
    expect_events(&mut poller, &mut events, last);
}

/// A wrapper function around `expect_events` to check that elapsed time doesn't
/// exceed `max_elapsed`.
fn expect_events_elapsed(poller: &mut Poller, events: &mut Events, max_elapsed: Duration, expected: Vec<Event>) {
    let start = Instant::now();
    expect_events(poller, events, expected);
    let elapsed = start.elapsed();
    assert!(elapsed < max_elapsed, "expected poll to return within {:?}, \
            it returned after: {:?}", max_elapsed, elapsed);
}
