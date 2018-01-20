use mio::event::{Event, Events};
use mio::poll::{Poll, PollOpt, Ready, Token};
use mio::registration::{Registration, RegistrationGone};

use expect_events;

#[test]
fn simple() {
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(128);

    let (mut registration, mut notifier) = Registration::new();
    poll.register(&mut registration, Token(0), Ready::READABLE, PollOpt::EDGE).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![]);

    // Ok.
    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Ok(true));
    // Should become only readable.
    assert_eq!(notifier.notify(&mut poll, Ready::READABLE | Ready::WRITABLE), Ok(true));
    // Not interested.
    assert_eq!(notifier.notify(&mut poll, Ready::WRITABLE), Ok(false));

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(Token(0), Ready::READABLE),
        Event::new(Token(0), Ready::READABLE),
    ]);
}

#[test]
fn registration_registering_notifications() {
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(128);

    let (mut registration, mut notifier) = Registration::new();

    // Notify first, which should fail.
    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Ok(false));

    // Now register and we shouldn't receive any events.
    poll.register(&mut registration, Token(123), Ready::READABLE, PollOpt::EDGE).unwrap();
    expect_events(&mut poll, &mut events, 1, vec![]);

    // After deregistering notifying should starting returning errors again.
    poll.deregister(&mut registration).unwrap();
    assert_eq!(notifier.interest(), Ok(Ready::empty()));
    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Ok(false));
}

#[test]
fn notify_after_dropping_registration() {
    let mut poll = Poll::new().unwrap();

    let (registration, mut notifier) = Registration::new();
    drop(registration);

    assert_eq!(notifier.notify(&mut poll, Ready::READABLE), Err(RegistrationGone));
    assert_eq!(notifier.interest(), Err(RegistrationGone));
}
