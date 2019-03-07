use std::time::Duration;

use log::error;

use mio_st::event::{self, Capacity, Source, Ready};
use mio_st::{Events, Event, Queue};

mod util;

use self::util::init;

#[test]
fn queue() {
    init();
    let mut queue = Queue::new();
    let mut events = Vec::new();

    assert_eq!(next_event_available(&mut queue), None);

    // Single event.
    let event = Event::new(event::Id(0), Ready::READABLE);
    queue.add(event);
    assert_eq!(next_event_available(&mut queue), Some(Duration::from_millis(0)));
    expect_events(&mut queue, &mut events, vec![event]);

    // Multiple events.
    queue.add(Event::new(event::Id(0), Ready::READABLE));
    queue.add(Event::new(event::Id(0), Ready::WRITABLE));
    queue.add(Event::new(event::Id(0), Ready::READABLE | Ready::WRITABLE));
    queue.add(Event::new(event::Id(1), Ready::READABLE | Ready::WRITABLE | Ready::ERROR));
    assert_eq!(next_event_available(&mut queue), Some(Duration::from_millis(0)));
    expect_events(&mut queue, &mut events, vec![
        Event::new(event::Id(0), Ready::READABLE),
        Event::new(event::Id(0), Ready::WRITABLE),
        Event::new(event::Id(0), Ready::READABLE | Ready::WRITABLE),
        Event::new(event::Id(1), Ready::READABLE | Ready::WRITABLE | Ready::ERROR),
    ]);
}

#[test]
fn queue_events_capacity() {
    init();

    struct EventsCapacity(Capacity, usize);

    impl Events for EventsCapacity {
        fn capacity_left(&self) -> Capacity {
            self.0
        }

        fn add(&mut self, _event: Event) {
            self.1 += 1;
        }
    }

    let mut queue = Queue::new();
    let event = Event::new(event::Id(0), Ready::READABLE);
    queue.add(event);
    queue.add(event);

    let mut events = EventsCapacity(Capacity::Limited(0), 0);
    Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
    assert_eq!(events.1, 0); // Shouldn't have grow.

    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
    assert_eq!(events.1, 1);

    let mut events = EventsCapacity(Capacity::Limited(1), 0);
    Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
    assert_eq!(events.1, 1);

    let mut events = EventsCapacity(Capacity::Limited(100), 0);
    queue.add(event);
    queue.add(event);
    Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
    assert_eq!(events.1, 2);

    let mut events = EventsCapacity(Capacity::Growable, 0);
    queue.add(event);
    queue.add(event);
    Source::<_, ()>::poll(&mut queue, &mut events).unwrap();
    assert_eq!(events.1, 2);
}

/// Get the next available event with having to worry about the generic
/// parameters.
fn next_event_available(queue: &mut Queue) -> Option<Duration> {
    Source::<Vec<Event>, ()>::next_event_available(queue)
}

/// Poll `Queue` for events.
fn expect_events(queue: &mut Queue, events: &mut Vec<Event>, mut expected: Vec<Event>) {
    events.clear();
    Source::<_, ()>::poll(queue, events)
        .expect("unable to poll user space queue");

    for event in events.drain(..) {
        let index = expected.iter()
            .position(|expected| {
                event.id() == expected.id() &&
                event.readiness().contains(expected.readiness())
            });

        if let Some(index) = index {
            expected.swap_remove(index);
        } else {
            // Must accept sporadic events.
            error!("got unexpected event: {:?}", event);
        }
    }

    assert!(expected.is_empty(), "the following expected events were not found: {:?}", expected);
}
