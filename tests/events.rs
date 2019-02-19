
/*
use crate::event::{Event, EventedId, Ready};
use crate::event::Ready;
use crate::event::EventedId;

#[test]
fn event() {
    let id = EventedId(0);
    assert_eq!(EventedId::from(0), EventedId(0));
    assert_eq!(usize::from(id), 0);
    assert_eq!(id.0, 0);
    assert_eq!(id, id.clone());

    let max_value = usize::max_value();
    let id = EventedId(max_value);
    assert_eq!(EventedId::from(max_value), EventedId(max_value));
    assert_eq!(usize::from(id), max_value);
    assert_eq!(id.0, max_value);
    assert_eq!(id, id.clone());
}

#[test]
fn event() {
    let event = Event::new(EventedId(0), Ready::READABLE);
    assert_eq!(event.id(), EventedId(0));
    assert_eq!(event.readiness(), Ready::READABLE);
}

#[test]
fn equality() {
    let event = Event::new(EventedId(0), Ready::WRITABLE);
    assert_eq!(event, event.clone());

    // Same
    let event2 = Event::new(EventedId(0), Ready::WRITABLE);
    assert_eq!(event, event2);

    // Different id.
    let event3 = Event::new(EventedId(1), Ready::WRITABLE);
    assert_ne!(event, event3);

    // Different readiness.
    let event4 = Event::new(EventedId(0), Ready::READABLE);
    assert_ne!(event, event4);
}

#[test]
fn is_tests() {
    assert!(!Ready::empty().is_readable());
    assert!(!Ready::empty().is_writable());
    assert!(!Ready::empty().is_error());
    assert!(!Ready::empty().is_timer());
    #[cfg(unix)]
    assert!(!Ready::empty().is_hup());

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
fn contains() {
    assert!(!Ready::READABLE.contains(Ready::READABLE | Ready::WRITABLE));
    assert!(!Ready::WRITABLE.contains(Ready::READABLE | Ready::WRITABLE));
    assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::READABLE | Ready::WRITABLE));
    assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::READABLE));
}

#[test]
fn bit_or() {
    let readiness = Ready::READABLE | Ready::WRITABLE | Ready::ERROR;
    assert!(readiness.is_readable());
    assert!(readiness.is_writable());
    assert!(readiness.is_error());
    assert!(!readiness.is_timer());
    #[cfg(unix)]
    assert!(!readiness.is_hup());
}

#[test]
fn bit_or_assign() {
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
fn fmt_debug() {
    assert_eq!(format!("{:?}", Ready::READABLE), "READABLE");
    assert_eq!(format!("{:?}", Ready::WRITABLE), "WRITABLE");
    assert_eq!(format!("{:?}", Ready::ERROR), "ERROR");
    assert_eq!(format!("{:?}", Ready::TIMER), "TIMER");
    #[cfg(unix)]
    assert_eq!(format!("{:?}", Ready::HUP), "HUP");

    assert_eq!(format!("{:?}", Ready::empty()), "(empty)");

    assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE), "READABLE | WRITABLE");
    assert_eq!(format!("{:?}", Ready::ERROR | Ready::TIMER), "ERROR | TIMER");
    assert_eq!(format!("{:?}", Ready::READABLE | Ready::WRITABLE | Ready::ERROR | Ready::TIMER),
        "READABLE | WRITABLE | ERROR | TIMER");
}
*/
