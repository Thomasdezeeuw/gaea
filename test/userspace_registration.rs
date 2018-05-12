use mio_st::event::{Event, EventedId};
use mio_st::poll::{PollOpt, Ready};
use mio_st::registration::{NotifyError, Registration, RegistrationGone};

use {expect_events, init_with_poll};

#[test]
fn no_notifications_before_registering() {
    let (mut poll, mut events) = init_with_poll();
    let (mut _registration, mut notifier) = Registration::new();

    // Not notifications with registering first.
    assert_eq!(notifier.notify(Ready::WRITABLE), Err(NotifyError::NotRegistered));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn incorrect_readiness() {
    let (mut poll, mut events) = init_with_poll();
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();

    // Incorrect readiness.
    assert_eq!(notifier.notify(Ready::empty()), Err(NotifyError::EmptyReadiness));
    assert_eq!(notifier.notify(Ready::WRITABLE), Err(NotifyError::NoInterest));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn no_notifications_after_registration_dropped() {
    let (mut poll, mut events) = init_with_poll();
    let (registration, mut notifier) = Registration::new();

    // No more notifications after it's dropped.
    drop(registration);
    assert_eq!(notifier.notify(Ready::READABLE), Err(NotifyError::RegistrationGone));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn notifier_interests() {
    let (mut poll, mut events) = init_with_poll();
    let (mut registration, mut notifier) = Registration::new();

    // Before registering this should be empty.
    assert_eq!(notifier.interests(), Ok(Ready::empty()));

    // After registering it should be an actual value.
    poll.register(&mut registration, EventedId(0), Ready::WRITABLE, PollOpt::Edge).unwrap();
    assert_eq!(notifier.interests(), Ok(Ready::WRITABLE));

    // No point in getting interests after the registration is dropped.
    drop(registration);
    assert_eq!(notifier.interests(), Err(RegistrationGone));

    expect_events(&mut poll, &mut events, 1, vec![]);
}

#[test]
fn notify() {
    let (mut poll, mut events) = init_with_poll();
    let (mut registration, mut notifier) = Registration::new();

    poll.register(&mut registration, EventedId(0), Ready::READABLE, PollOpt::Edge).unwrap();

    // Ok.
    notifier.notify(Ready::READABLE).unwrap();

    // Should become only readable.
    notifier.notify(Ready::READABLE | Ready::WRITABLE).unwrap();
    notifier.notify(Ready::all()).unwrap();

    expect_events(&mut poll, &mut events, 1, vec![
        Event::new(EventedId(0), Ready::READABLE),
        Event::new(EventedId(0), Ready::READABLE),
        Event::new(EventedId(0), Ready::READABLE),
    ]);
}
