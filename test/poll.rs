use std::time::Instant;

use mio_st::event::{Event, EventedId};
use mio_st::poll::{PollOpt, Ready};
use mio_st::registration::Registration;

use init_with_poll;

// TODO: add tests for both TcpStream and TcpListener:
// reregistering and
// deregistering.

#[test]
fn polling_dont_expand_events() {
    const EVENTS_SIZE: usize = 1;
    let (mut poll, mut events) = init_with_poll(EVENTS_SIZE);

    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();

    notifier.notify(&mut poll, Ready::READABLE).unwrap();
    notifier.notify(&mut poll, Ready::READABLE).unwrap();
    notifier.notify(&mut poll, Ready::READABLE).unwrap();

    poll.add_deadline(EventedId(1), Instant::now()).unwrap();
    poll.add_deadline(EventedId(2), Instant::now()).unwrap();
    poll.add_deadline(EventedId(3), Instant::now()).unwrap();

    // Since we created events with a capacity of 1, it shouldn't expand that
    // capacity.
    for n in 0..6 {
        poll.poll(&mut events, None).unwrap();

        assert_eq!(events.len(), EVENTS_SIZE);
        match n {
            0 ... 2 => {
                let id = n + 1;
                assert_eq!((&mut events).next().unwrap(), Event::new(EventedId(id), Ready::TIMER));
            },
            _ => assert_eq!((&mut events).next().unwrap(), Event::new(EventedId(0), Ready::READABLE)),
        }
    }
}
