use std::thread;
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::poll::Awakener;

mod util;

use self::util::{expect_events, init_with_poller};

#[test]
fn awakener() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    let awakener = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");

    let handle = thread::spawn(move || {
        awakener.wake().expect("unable to wake");
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    handle.join().unwrap();
}

#[test]
fn awakener_try_clone() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    let awakener1 = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");
    let awakener2 = awakener1.try_clone()
        .expect("unable to clone awakener");

    let handle1 = thread::spawn(move || {
        awakener1.wake().expect("unable to wake");
    });
    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        awakener2.wake().expect("unable to wake");
    });

    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn awakener_drain() {
    let (mut poller, mut events) = init_with_poller();

    let event_id = EventedId(10);
    let awakener = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");

    for _ in 0..3 {
        awakener.wake().expect("unable to wake");
    }

    // Depending on the platform we can get 3 or 1 event.
    expect_events(&mut poller, &mut events, vec![
        Event::new(event_id, Ready::READABLE),
    ]);

    awakener.drain().expect("unable to drain awakener");
}

#[test]
fn awakener_drain_empty() {
    let (mut poller, _) = init_with_poller();

    let event_id = EventedId(10);
    let awakener = Awakener::new(&mut poller, event_id)
        .expect("unable to create awakener");

    awakener.drain().expect("unable to drain awakener");
}