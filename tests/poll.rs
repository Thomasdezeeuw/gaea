use std::io;
use std::thread::sleep;
use std::time::{Duration, Instant};

use mio_st::{Events, poll};
use mio_st::event::{BlockingSource, Source};

mod util;

use self::util::{TIMEOUT_MARGIN, init};

struct SleepySource;

impl<Evts> Source<Evts> for SleepySource
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        None
    }

    fn poll(&mut self, events: &mut Evts) -> io::Result<()> {
        self.blocking_poll(events, Some(Duration::from_millis(0)))
    }
}

impl<Evts> BlockingSource<Evts> for SleepySource
    where Evts: Events,
{
    fn blocking_poll(&mut self, _events: &mut Evts, timeout: Option<Duration>) -> io::Result<()> {
        let timeout = timeout.expect("SleepySource needs a timeout");
        sleep(timeout);
        Ok(())
    }
}

struct AvailableSource(Duration);

impl<Evts> Source<Evts> for AvailableSource
    where Evts: Events,
{
    fn next_event_available(&self) -> Option<Duration> {
        Some(self.0)
    }

    fn poll(&mut self, _events: &mut Evts) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn poll_determine_timeout() {
    init();

    let mut events = Vec::new();
    let timeout = Duration::from_millis(10);

    let start = Instant::now();
    poll(&mut SleepySource, &mut [&mut AvailableSource(timeout)], &mut events, None).unwrap();
    assert!(events.is_empty());
    let duration = start.elapsed();
    #[cfg(not(feature="disable_test_deadline"))]
    assert!(duration >= timeout && duration <= timeout + TIMEOUT_MARGIN,
        "blocking time incorrect: {:?}, wanted: >= {:?} and >= {:?}.", duration, timeout, timeout + TIMEOUT_MARGIN);

    let start = Instant::now();
    poll(&mut SleepySource, &mut [&mut AvailableSource(Duration::from_secs(1))], &mut events, Some(timeout)).unwrap();
    assert!(events.is_empty());
    let duration = start.elapsed();
    #[cfg(not(feature="disable_test_deadline"))]
    assert!(duration >= timeout && duration <= timeout + TIMEOUT_MARGIN,
        "blocking time incorrect: {:?}, wanted: >= {:?} and >= {:?}.", duration, timeout, timeout + TIMEOUT_MARGIN);
}
