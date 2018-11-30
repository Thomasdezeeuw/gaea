use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;

use mio_st::event::{Event, EventedId, Ready};
use mio_st::poll::{Interests, PollOption, Poller};
use mio_st::unix::{new_pipe, Sender, Receiver};

mod util;

use self::util::{expect_events, init, init_with_poller};

const SENDER_ID: EventedId = EventedId(0);
const RECEIVER_ID: EventedId = EventedId(1);

const DATA: &'static [u8] = b"Hello world!";

#[test]
fn unix_pipe() {
    let (mut poller, mut events) = init_with_poller();

    let (mut sender, mut receiver) = new_pipe().expect("can't create pipe");

    poller.register(&mut sender, SENDER_ID, Sender::INTERESTS, PollOption::Level)
        .expect("can't register sender");
    poller.register(&mut receiver, RECEIVER_ID, Receiver::INTERESTS, PollOption::Level)
        .expect("can't register receiver");

    expect_events(&mut poller, &mut events, vec![
        Event::new(SENDER_ID, Ready::WRITABLE),
    ]);

    assert_eq!(sender.write(DATA).unwrap(), DATA.len());

    // Ensure that the sending half is ready for writing again.
    sleep(Duration::from_millis(10));

    expect_events(&mut poller, &mut events, vec![
        Event::new(RECEIVER_ID, Ready::READABLE),
        Event::new(SENDER_ID, Ready::WRITABLE),
    ]);

    let mut buf = [0; 20];
    assert_eq!(receiver.read(&mut buf).unwrap(), DATA.len());
    assert_eq!(buf[0..DATA.len()], DATA[..]);
}

#[test]
#[should_panic(expected = "receiving end of a pipe can never be written")]
fn receiver_writable_interests() {
    init();

    let (_, mut receiver) = new_pipe().expect("can't create pipe");

    let mut poller = Poller::new().unwrap();
    poller.register(&mut receiver, RECEIVER_ID, Interests::WRITABLE, PollOption::Level)
        .unwrap();
}

#[test]
#[should_panic(expected = "sending end of a pipe can never be read")]
fn sender_readable_interests() {
    init();

    let (mut sender, _) = new_pipe().expect("can't create pipe");

    let mut poller = Poller::new().unwrap();
    poller.register(&mut sender, SENDER_ID, Interests::READABLE, PollOption::Level)
        .unwrap();
}
