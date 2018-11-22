use mio_st::event::{Event, EventedId, Ready};

use crate::{expect_events, init_with_poll};

#[test]
fn notify() {
    let (mut poll, mut events) = init_with_poll();

    poll.notify(EventedId(0), Ready::READABLE).unwrap();
    poll.notify(EventedId(0), Ready::WRITABLE).unwrap();
    poll.notify(EventedId(0), Ready::READABLE | Ready::WRITABLE).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::READABLE),
        Event::new(EventedId(0), Ready::WRITABLE),
        Event::new(EventedId(0), Ready::READABLE | Ready::WRITABLE),
    ]);
}
