use std::io;
use std::time::{Duration, Instant};

use mio_st::event::{self, BlockingSource, Source, Event};
use mio_st::os::{Evented, Interests, PollOption, OsQueue};

mod util;

use self::util::{TIMEOUT_MARGIN, assert_error, init, init_with_os_queue};

struct TestEvented {
    registrations: Vec<(event::Id, Interests, PollOption)>,
    reregistrations: Vec<(event::Id, Interests, PollOption)>,
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
    fn register(&mut self, _os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        self.registrations.push((id, interests, opt));
        Ok(())
    }

    fn reregister(&mut self, _os_queue: &mut OsQueue, id: event::Id, interests: Interests, opt: PollOption) -> io::Result<()> {
        self.reregistrations.push((id, interests, opt));
        Ok(())
    }

    fn deregister(&mut self, _os_queue: &mut OsQueue) -> io::Result<()> {
        self.deregister_count += 1;
        Ok(())
    }
}

#[test]
fn os_queue_registration() {
    init();
    let mut os_queue = OsQueue::new().expect("unable to create OsQueue");

    let mut handle = TestEvented::new();
    let id = event::Id(0);
    let interests = Interests::READABLE;
    let opt = PollOption::Edge;
    os_queue.register(&mut handle, id, interests, opt)
        .expect("unable to register evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.registrations.get(0), Some(&(id, interests, opt)));
    assert!(handle.reregistrations.is_empty());
    assert_eq!(handle.deregister_count, 0);

    let re_id = event::Id(0);
    let re_interests = Interests::READABLE;
    let re_opt = PollOption::Edge;
    os_queue.reregister(&mut handle, re_id, re_interests, re_opt)
        .expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.reregistrations.get(0), Some(&(re_id, re_interests, re_opt)));
    assert_eq!(handle.deregister_count, 0);

    os_queue.deregister(&mut handle).expect("unable to reregister evented handle");
    assert_eq!(handle.registrations.len(), 1);
    assert_eq!(handle.reregistrations.len(), 1);
    assert_eq!(handle.deregister_count, 1);
}

struct ErroneousTestEvented;

impl Evented for ErroneousTestEvented {
    fn register(&mut self, _os_queue: &mut OsQueue, _id: event::Id, _interests: Interests, _opt: PollOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "register"))
    }

    fn reregister(&mut self, _os_queue: &mut OsQueue, _id: event::Id, _interests: Interests, _opt: PollOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "reregister"))
    }

    fn deregister(&mut self, _os_queue: &mut OsQueue) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "deregister"))
    }
}

#[test]
fn os_queue_erroneous_registration() {
    init();
    let mut os_queue = OsQueue::new().expect("unable to create OsQueue");

    let mut handle = ErroneousTestEvented;
    let id = event::Id(0);
    let interests = Interests::READABLE;
    let opt = PollOption::Edge;
    assert_error(os_queue.register(&mut handle, id, interests, opt), "register");
    assert_error(os_queue.reregister(&mut handle, id, interests, opt), "reregister");
    assert_error(os_queue.deregister(&mut handle), "deregister");
}

// NOTE: the `BlockingSource` and `Source` implementation are testing more in
// the TCP and UDP tests.

#[test]
fn os_queue_empty_source() {
    let (mut os_queue, mut events) = init_with_os_queue();

    assert_eq!(Source::<Vec<Event>>::next_event_available(&mut os_queue), None);

    os_queue.poll(&mut events).unwrap();
    assert!(events.is_empty(), "unexpected events");

    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    os_queue.blocking_poll(&mut events, Some(timeout)).unwrap();
    assert!(start.elapsed() < timeout + TIMEOUT_MARGIN, "polling took too long.");
    assert!(events.is_empty(), "unexpected events");
}
