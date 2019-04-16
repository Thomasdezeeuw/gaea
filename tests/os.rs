use std::io::{self, Write};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use mio_st::event::{self, Capacity, Event, Ready};
use mio_st::os::{Awakener, Evented, Interests, OsQueue, RegisterOption};
use mio_st::unix::new_pipe;

mod util;

use self::util::{assert_error, expect_events, init, init_with_os_queue, EventsCapacity, TIMEOUT_MARGIN};

struct TestEvented {
    registrations: Vec<(event::Id, Interests, RegisterOption)>,
    reregistrations: Vec<(event::Id, Interests, RegisterOption)>,
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
    fn register(&mut self, _os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.registrations.push((id, interests, opt));
        Ok(())
    }

    fn reregister(&mut self, _os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        self.reregistrations.push((id, interests, opt));
        Ok(())
    }

    fn deregister(&mut self, _os_queue: &mut OsQueue) -> io::Result<()> {
        self.deregister_count += 1;
        Ok(())
    }
}

#[test]
fn os_queue_registration() {
    init();
    let mut os_queue = OsQueue::new().expect("unable to create OsQueue");

    let mut handle = TestEvented::new();
    let id = event::Id(0);
    let interests = Interests::READABLE;
    let opt = RegisterOption::EDGE;
    os_queue.register(&mut handle, id, interests, opt)
        .expect("unable to register evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.registrations.get(0), Some(&(id, interests, opt)));
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);

    let re_id = event::Id(0);
    let re_interests = Interests::READABLE;
    let re_opt = RegisterOption::EDGE;
    os_queue.reregister(&mut handle, re_id, re_interests, re_opt)
        .expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.reregistrations.get(0), Some(&(re_id, re_interests, re_opt)));
    assert_eq!(handle.deregister_count, 0);

    os_queue.deregister(&mut handle).expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.deregister_count, 1);
}

struct ErroneousTestEvented;

impl Evented for ErroneousTestEvented {
    fn register(&mut self, _os_queue: &mut OsQueue, _id: event::Id, _interests: Interests, _opt: RegisterOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "register"))
    }

    fn reregister(&mut self, _os_queue: &mut OsQueue, _id: event::Id, _interests: Interests, _opt: RegisterOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "reregister"))
    }

    fn deregister(&mut self, _os_queue: &mut OsQueue) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "deregister"))
    }
}

#[test]
fn os_queue_erroneous_registration() {
    init();
    let mut os_queue = OsQueue::new().expect("unable to create OsQueue");

    let mut handle = ErroneousTestEvented;
    let id = event::Id(0);
    let interests = Interests::READABLE;
    let opt = RegisterOption::EDGE;
    assert_error(os_queue.register(&mut handle, id, interests, opt), "register");
    assert_error(os_queue.reregister(&mut handle, id, interests, opt), "reregister");
    assert_error(os_queue.deregister(&mut handle), "deregister");
}

// NOTE: the `event::Source` implementation is tested more thoroughly in the TCP
// and UDP tests.

#[test]
fn os_queue_empty_source() {
    let (mut os_queue, mut events) = init_with_os_queue();

    assert_eq!(event::Source::<Vec<Event>, io::Error>::next_event_available(&mut os_queue), None);

    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert!(events.is_empty(), "unexpected events");

    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    event::Source::<_, io::Error>::blocking_poll(&mut os_queue, &mut events, Some(timeout)).unwrap();
    #[cfg(not(feature="disable_test_deadline"))]
    assert!(start.elapsed() <= timeout + TIMEOUT_MARGIN,
        "polling took too long: {:?}, wanted: <= {:?}.", start.elapsed(), timeout + TIMEOUT_MARGIN);
    assert!(events.is_empty(), "unexpected events");
}

#[test]
fn queue_events_capacity() {
    init();
    let mut os_queue = OsQueue::new().unwrap();

    // Add two events to the OS queue.
    let awakener = Awakener::new(&mut os_queue, event::Id(0)).unwrap();
    awakener.wake().unwrap();
    let (mut sender, mut receiver) = new_pipe().unwrap();
    let opt = RegisterOption::ONESHOT | RegisterOption::LEVEL;
    os_queue.register(&mut sender, event::Id(1), Interests::WRITABLE, opt).unwrap();

    let mut events = EventsCapacity(Capacity::Limited(0), 0);
    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert_eq!(events.1, 0); // Shouldn't have grow.

    // The events should remain in the OS queue and be return in the following
    // two poll calls.
    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert_eq!(events.1, 1);

    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert_eq!(events.1, 1);

    // Add two more events.
    os_queue.reregister(&mut sender, event::Id(1), Interests::WRITABLE, opt).unwrap();
    awakener.wake().unwrap();

    let mut events = EventsCapacity(Capacity::Limited(100), 0);
    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert_eq!(events.1, 2);

    // Add three more events.
    os_queue.reregister(&mut sender, event::Id(1), Interests::WRITABLE, opt).unwrap();
    os_queue.register(&mut receiver, event::Id(1), Interests::READABLE, opt).unwrap();
    sender.write(b"Hello").unwrap();
    awakener.wake().unwrap();

    let mut events = EventsCapacity(Capacity::Growable, 0);
    event::Source::<_, io::Error>::poll(&mut os_queue, &mut events).unwrap();
    assert_eq!(events.1, 3);
}

#[test]
fn awakener() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let event_id = event::Id(10);
    // Keep `awakener` alive on this thread and create a new awakener to move
    // to the other thread.
    let awakener = Awakener::new(&mut os_queue, event_id)
        .expect("unable to create awakener");

    // Waking on the same thread.
    awakener.wake().expect("unable to wake");
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    // Multiple wakes between polls.
    for _ in 0..3 {
        awakener.wake().expect("unable to wake");
    }
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    // Waking on another thread.
    let awakener1 = awakener.try_clone()
        .expect("unable to clone awakener");
    let handle = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    event::Source::<_, io::Error>::blocking_poll(&mut os_queue, &mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());

    handle.join().unwrap();
}

#[test]
fn awakener_try_clone() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let event_id = event::Id(10);
    // Keep `awakener` alive on this thread and create two new awakeners to move
    // to the other threads.
    let awakener = Awakener::new(&mut os_queue, event_id)
        .expect("unable to create awakener");
    let awakener1 = awakener.try_clone()
        .expect("unable to clone awakener");
    let awakener2 = awakener1.try_clone()
        .expect("unable to clone awakener");

    let handle1 = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });

    handle1.join().unwrap();
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        awakener2.wake().expect("unable to wake");
    });

    handle2.join().unwrap();
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    event::Source::<_, io::Error>::blocking_poll(&mut os_queue, &mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());
}

#[test]
fn awakener_multiple_wakeups() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let event_id = event::Id(10);
    let awakener = Awakener::new(&mut os_queue, event_id)
        .expect("unable to create awakener");
    let awakener1 = awakener.try_clone()
        .expect("unable to clone awakener");
    let awakener2 = awakener1.try_clone()
        .expect("unable to clone awakener");

    let handle1 = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });

    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let handle2 = thread::spawn(move || {
        barrier2.wait();
        awakener2.wake().expect("unable to wake");
    });

    // Receive the event from thread 1.
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    // Unblock thread 2.
    barrier.wait();

    // Now we need to receive another event from thread 2.
    expect_events(&mut os_queue, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    event::Source::<_, io::Error>::blocking_poll(&mut os_queue, &mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());

    handle1.join().unwrap();
    handle2.join().unwrap();
}
