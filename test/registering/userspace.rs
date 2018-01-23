use std::error::Error;

use mio_st::event::{Event, EventedId};
use mio_st::poll::{Ready, PollOpt};
use mio_st::registration::{Registration, NotifyError};

use {expect_events, init_with_poll};

#[test]
fn registering_deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut registration).unwrap();

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::NotRegistered));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_reregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.reregister(&mut registration, EventedId(1), Ready::WRITABLE, PollOpt::Edge).unwrap();

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::NoInterest));
    notifier.notify(&mut poll, Ready::WRITABLE).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::WRITABLE),
    ]);
}

#[test]
fn registering_reregistering_deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.reregister(&mut registration, EventedId(1), Ready::WRITABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut registration).unwrap();

    assert_eq!(notifier.notify(&mut poll, Ready::WRITABLE), Err(NotifyError::NotRegistered));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_deregistering_registering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    poll.deregister(&mut registration).unwrap();
    poll.reregister(&mut registration, EventedId(1), Ready::WRITABLE, PollOpt::Edge).unwrap();

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::NoInterest));
    notifier.notify(&mut poll, Ready::WRITABLE).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(1), Ready::WRITABLE),
    ]);
}

#[test]
fn reregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    let result = poll.reregister(&mut registration, EventedId(1), Ready::WRITABLE, PollOpt::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot reregister"));

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::NotRegistered));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn deregistering() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    let result = poll.deregister(&mut registration);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot deregister"));

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(NotifyError::NotRegistered));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn registering_twice() {
    let (mut poll, mut events) = init_with_poll(8);
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();
    let result = poll.register(&mut registration, EventedId(1), Ready::WRITABLE, PollOpt::Edge);
    assert!(result.is_err());
    assert!(result.unwrap_err().description().contains("cannot register"));

    expect_events(&mut poll, &mut events, 1, vec![]);
}
