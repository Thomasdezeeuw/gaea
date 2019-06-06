use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;

use gaea::event;
use gaea::event::{Event, Ready};
use gaea::os::{Interests, OsQueue, RegisterOption};
use gaea::unix::{new_pipe, Receiver, Sender};

mod util;

use self::util::{expect_events, init, init_with_os_queue};

const SENDER_ID: event::Id = event::Id(0);
const RECEIVER_ID: event::Id = event::Id(1);

const DATA: &[u8] = b"Hello world!";

#[test]
fn unix_pipe() {
    let (mut os_queue, mut events) = init_with_os_queue();

    let (mut sender, mut receiver) = new_pipe().expect("can't create pipe");

    os_queue.register(&mut sender, SENDER_ID, Sender::INTERESTS, RegisterOption::LEVEL)
        .expect("can't register sender");
    os_queue.register(&mut receiver, RECEIVER_ID, Receiver::INTERESTS, RegisterOption::LEVEL)
        .expect("can't register receiver");

    expect_events(&mut os_queue, &mut events, vec![
        Event::new(SENDER_ID, Ready::WRITABLE),
    ]);

    assert_eq!(sender.write(DATA).unwrap(), DATA.len());

    // Ensure that the sending half is ready for writing again.
    sleep(Duration::from_millis(10));

    expect_events(&mut os_queue, &mut events, vec![
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

    let mut os_queue = OsQueue::new().unwrap();
    os_queue.register(&mut receiver, RECEIVER_ID, Interests::WRITABLE, RegisterOption::LEVEL)
        .unwrap();
}

#[test]
#[should_panic(expected = "sending end of a pipe can never be read")]
fn sender_readable_interests() {
    init();

    let (mut sender, _) = new_pipe().expect("can't create pipe");

    let mut os_queue = OsQueue::new().unwrap();
    os_queue.register(&mut sender, SENDER_ID, Interests::READABLE, RegisterOption::LEVEL)
        .unwrap();
}
