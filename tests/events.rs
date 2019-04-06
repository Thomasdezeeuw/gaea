use mio_st::event::{self, Capacity, Event, Ready, Sink};

#[test]
fn events_vec() {
    let mut events = Vec::new();

    assert_eq!(events.capacity_left(), Capacity::Growable);

    let event = Event::new(event::Id(0), Ready::READABLE);
    events.push(event);
    assert_eq!(events.pop(), Some(event));
}

#[test]
fn event() {
    let event = Event::new(event::Id(0), Ready::READABLE);
    assert_eq!(event.id(), event::Id(0));
    assert_eq!(event.readiness(), Ready::READABLE);
}

#[test]
fn event_equality() {
    let event = Event::new(event::Id(0), Ready::WRITABLE);
    assert_eq!(event, event.clone());

    // Same
    let event2 = Event::new(event::Id(0), Ready::WRITABLE);
    assert_eq!(event, event2);

    // Different id.
    let event3 = Event::new(event::Id(1), Ready::WRITABLE);
    assert_ne!(event, event3);

    // Different readiness.
    let event4 = Event::new(event::Id(0), Ready::READABLE);
    assert_ne!(event, event4);
}

#[test]
fn ready_contains() {
    assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::READABLE));
    assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::WRITABLE));
    assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::READABLE | Ready::WRITABLE));
    assert!(!Ready::READABLE.contains(Ready::READABLE | Ready::WRITABLE));
    assert!(!Ready::WRITABLE.contains(Ready::READABLE | Ready::WRITABLE));
}

#[test]
fn ready_is_tests() {
    assert!(!Ready::EMPTY.is_readable());
    assert!(!Ready::EMPTY.is_writable());
    assert!(!Ready::EMPTY.is_error());
    assert!(!Ready::EMPTY.is_timer());
    #[cfg(unix)]
    assert!(!Ready::EMPTY.is_hup());

    assert!(Ready::READABLE.is_readable());
    assert!(!Ready::READABLE.is_writable());
    assert!(!Ready::READABLE.is_error());
    assert!(!Ready::READABLE.is_timer());
    #[cfg(unix)]
    assert!(!Ready::READABLE.is_hup());

    assert!(!Ready::WRITABLE.is_readable());
    assert!(Ready::WRITABLE.is_writable());
    assert!(!Ready::WRITABLE.is_error());
    assert!(!Ready::WRITABLE.is_timer());
    #[cfg(unix)]
    assert!(!Ready::WRITABLE.is_hup());

    assert!(!Ready::ERROR.is_readable());
    assert!(!Ready::ERROR.is_writable());
    assert!(Ready::ERROR.is_error());
    assert!(!Ready::ERROR.is_timer());
    #[cfg(unix)]
    assert!(!Ready::ERROR.is_hup());

    assert!(!Ready::TIMER.is_readable());
    assert!(!Ready::TIMER.is_writable());
    assert!(!Ready::TIMER.is_error());
    assert!(Ready::TIMER.is_timer());
    #[cfg(unix)]
    assert!(!Ready::TIMER.is_hup());

    #[cfg(unix)]
    {
        assert!(!Ready::HUP.is_readable());
        assert!(!Ready::HUP.is_writable());
        assert!(!Ready::HUP.is_error());
        assert!(!Ready::HUP.is_timer());
        assert!(Ready::HUP.is_hup());
    }
}

#[test]
fn ready_bit_or() {
    let readiness = Ready::READABLE | Ready::WRITABLE | Ready::ERROR;
    assert!(readiness.is_readable());
    assert!(readiness.is_writable());
    assert!(readiness.is_error());
    assert!(!readiness.is_timer());
    #[cfg(unix)]
    assert!(!readiness.is_hup());
}

#[test]
fn ready_bit_or_assign() {
    let mut readiness = Ready::READABLE;
    readiness |= Ready::WRITABLE;
    assert!(readiness.is_readable());
    assert!(readiness.is_writable());
    assert!(!readiness.is_error());
    assert!(!readiness.is_timer());
    #[cfg(unix)]
    assert!(!readiness.is_hup());
}

#[test]
fn ready_fmt_debug() {
    assert_eq!(format!("{:?}", Ready::EMPTY), "(empty)");
    assert_eq!(format!("{:?}", Ready::READABLE), "READABLE");
    assert_eq!(format!("{:?}", Ready::WRITABLE), "WRITABLE");
    assert_eq!(format!("{:?}", Ready::ERROR), "ERROR");
    assert_eq!(format!("{:?}", Ready::TIMER), "TIMER");
    #[cfg(unix)]
    assert_eq!(format!("{:?}", Ready::HUP), "HUP");

    assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE), "READABLE | WRITABLE");
    assert_eq!(format!("{:?}", Ready::ERROR | Ready::TIMER), "ERROR | TIMER");
    assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE | Ready::ERROR | Ready::TIMER),
        "READABLE | WRITABLE | ERROR | TIMER");
}

#[test]
fn id() {
    let id = event::Id(0);
    assert_eq!(event::Id::from(0), event::Id(0));
    assert_eq!(usize::from(id), 0);
    assert_eq!(id.0, 0);
    assert_eq!(id, id.clone());

    let max_value = usize::max_value();
    let id = event::Id(max_value);
    assert_eq!(event::Id::from(max_value), event::Id(max_value));
    assert_eq!(usize::from(id), max_value);
    assert_eq!(id.0, max_value);
    assert_eq!(id, id.clone());
}

#[test]
fn id_fmt() {
    assert_eq!(format!("{:?}", event::Id(0)), "Id(0)");
    assert_eq!(format!("{:?}", event::Id(999)), "Id(999)");

    assert_eq!(format!("{}", event::Id(0)), "0");
    assert_eq!(format!("{}", event::Id(999)), "999");
}
