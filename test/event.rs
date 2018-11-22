use mio_st::event::{Event, EventedId, Ready};

use crate::init;

#[test]
fn evented_id() {
    init();

    let id = EventedId(10);
    assert_eq!(usize::from(id), 10);
    assert_eq!(id.0, 10);
    assert!(id.is_valid());

    let max_value = usize::max_value();
    let id = EventedId(max_value);
    assert_eq!(usize::from(id), max_value);
    assert_eq!(id.0, max_value);
    assert!(!id.is_valid());

    assert_eq!(EventedId::from(0), EventedId(0));
    assert_eq!(EventedId::from(max_value), EventedId(max_value));
}

#[test]
fn event() {
    init();

    let event = Event::new(EventedId(0), Ready::all());
    assert_eq!(event.id(), EventedId(0));
    assert_eq!(event.readiness(), Ready::all());
}
