use std::io;
use std::thread::sleep;
use std::time::{Duration, Instant};

use mio_st::event::{Event, EventedId, Evented, Ready};
use mio_st::poll::{Poller, PollOption, PollCalled};

mod util;

use self::util::{assert_error, expect_userspace_events, init, init_with_poller};

struct TestEvented {
    registrations: Vec<(EventedId, Ready, PollOption)>,
    reregistrations: Vec<(EventedId, Ready, PollOption)>,
    deregister_count: usize,
}

impl TestEvented {
    fn new() -> TestEvented {
        TestEvented {
            registrations: Vec::new(),
            reregistrations: Vec::new(),
            deregister_count: 0,
        }
    }
}

impl Evented for TestEvented {
    fn register(&mut self, _poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, _p: PollCalled) -> io::Result<()> {
        self.registrations.push((id, interests, opt));
        Ok(())
    }

    fn reregister(&mut self, _poll: &mut Poller, id: EventedId, interests: Ready, opt: PollOption, _p: PollCalled) -> io::Result<()> {
        self.reregistrations.push((id, interests, opt));
        Ok(())
    }

    fn deregister(&mut self, _poll: &mut Poller, _p: PollCalled) -> io::Result<()> {
        self.deregister_count += 1;
        Ok(())
    }
}

#[test]
fn poller_registration() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = TestEvented::new();
    let id = EventedId(0);
    let interests = Ready::READABLE;
    let opt = PollOption::Edge;
    poller.register(&mut handle, id, interests, opt)
        .expect("unable to register evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.registrations.get(0), Some(&(id, interests, opt)));
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);

    let re_id = EventedId(0);
    let re_interests = Ready::READABLE;
    let re_opt = PollOption::Edge;
    poller.reregister(&mut handle, re_id, re_interests, re_opt)
        .expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.reregistrations.get(0), Some(&(re_id, re_interests, re_opt)));
    assert_eq!(handle.deregister_count, 0);

    poller.deregister(&mut handle).expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.deregister_count, 1);
}

struct ErroneousTestEvented;

impl Evented for ErroneousTestEvented {
    fn register(&mut self, _poll: &mut Poller, _id: EventedId, _interests: Ready, _opt: PollOption, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "register"))
    }

    fn reregister(&mut self, _poll: &mut Poller, _id: EventedId, _interests: Ready, _opt: PollOption, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "reregister"))
    }

    fn deregister(&mut self, _poll: &mut Poller, _p: PollCalled) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "deregister"))
    }
}

#[test]
fn poller_erroneous_registration() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = ErroneousTestEvented;
    let id = EventedId(0);
    let interests = Ready::READABLE;
    let opt = PollOption::Edge;
    assert_error(poller.register(&mut handle, id, interests, opt), "register");
    assert_error(poller.reregister(&mut handle, id, interests, opt), "reregister");
    assert_error(poller.deregister(&mut handle), "deregister");
}

#[test]
fn poller_register_empty_interests() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let mut handle = TestEvented::new();
    let id = EventedId(0);
    let interests = Ready::empty();
    let opt = PollOption::Edge;
    assert_error(poller.register(&mut handle, id, interests, opt),
                 "registering with empty interests");

    assert_error(poller.reregister(&mut handle, id, interests, opt),
                 "registering with empty interests");

    assert!(handle.registrations.is_empty());
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);
}

#[test]
fn poller_notify() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    let ready = Ready::READABLE;
    poller.notify(id, ready).expect("unable to notify");
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, ready)]);
}

#[test]
fn poller_notify_empty_interests() {
    init();
    let mut poller = Poller::new().expect("unable to create Poller instance");

    let id = EventedId(0);
    let ready = Ready::empty();
    assert_error(poller.notify(id, ready), "registering with empty interests");
}

#[test]
fn poller_add_deadline() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    poller.add_deadline(id, Instant::now());
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, Ready::TIMER)]);

    let timeout = Duration::from_millis(50);
    poller.add_deadline(id, Instant::now() + timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);

    sleep(timeout);
    expect_userspace_events(&mut poller, &mut events, vec![Event::new(id, Ready::TIMER)]);
}

#[test]
fn poller_remove_deadline() {
    let (mut poller, mut events) = init_with_poller();

    let id = EventedId(0);
    let timeout = Duration::from_millis(50);
    poller.add_deadline(id, Instant::now() + timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);

    poller.remove_deadline(id);

    sleep(timeout);
    expect_userspace_events(&mut poller, &mut events, vec![]);
}
