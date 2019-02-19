use std::thread;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::poll::Awakener;

mod util;

use self::util::{expect_events, init_with_poller};

#[test]
fn awakener() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    // Keep `awakener` alive on this thread and create a new awakener to move
    // to the other thread.
    let awakener = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");

    // Waking on the same thread.
    for _ in 0..3 {
        awakener.wake().expect("unable to wake");
    }

    // Depending on the platform we can get 3 or 1 event.
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    // Waking on another thread.
    let awakener1 = awakener.try_clone()
        .expect("unable to clone awakener");
    let handle = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    poller.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());

    handle.join().unwrap();
}

#[test]
fn awakener_try_clone() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    // Keep `awakener` alive on this thread and create two new awakeners to move
    // to the other threads.
    let awakener = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");
    let awakener1 = awakener.try_clone()
        .expect("unable to clone awakener");
    let awakener2 = awakener1.try_clone()
        .expect("unable to clone awakener");

    let handle1 = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });

    handle1.join().unwrap();
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        awakener2.wake().expect("unable to wake");
    });

    handle2.join().unwrap();
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    poller.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());
}

#[test]
fn awakener_multiple_wakeups() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    let awakener = Awakener::new(&mut poller, event_id)
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
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    // Unblock thread 2.
    barrier.wait();

    // Now we need to receive another event from thread 2.
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    poller.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("unable to poll");
    assert!(events.is_empty());

    handle1.join().unwrap();
    handle2.join().unwrap();
}