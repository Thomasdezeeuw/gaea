use mio_st::event::{Event, EventedId, Ready};

use crate::init;

#[test]
fn event() {
    init();

    let event = Event::new(EventedId(0), Ready::all());
    assert_eq!(event.id(), EventedId(0));
    assert_eq!(event.readiness(), Ready::all());
}
