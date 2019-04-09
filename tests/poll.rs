use std::thread::sleep;
use std::time::{Duration, Instant};

use mio_st::{event, poll};

mod util;

use self::util::{init, TIMEOUT_MARGIN};

struct SleepySource;

impl<ES, E> event::Source<ES, E> for SleepySource
    where ES: event::Sink,
{
    fn next_event_available(&self) -> Option<Duration> {
        None
    }

    fn poll(&mut self, event_sink: &mut ES) -> Result<(), E> {
        self.blocking_poll(event_sink, Some(Duration::from_millis(0)))
    }

    fn blocking_poll(&mut self, _event_sink: &mut ES, timeout: Option<Duration>) -> Result<(), E> {
        let timeout = timeout.expect("SleepySource needs a timeout");
        sleep(timeout);
        Ok(())
    }
}

struct AvailableSource(Duration);

impl<ES, E> event::Source<ES, E> for AvailableSource
    where ES: event::Sink,
{
    fn next_event_available(&self) -> Option<Duration> {
        Some(self.0)
    }

    fn poll(&mut self, _event_sink: &mut ES) -> Result<(), E> {
        Ok(())
    }
}

#[test]
fn poll_determine_timeout() {
    init();

    let mut events = Vec::new();
    let timeout = Duration::from_millis(10);

    let start = Instant::now();
    poll::<_, ()>(&mut [&mut SleepySource, &mut AvailableSource(timeout)], &mut events, None).unwrap();
    assert!(events.is_empty());
    let duration = start.elapsed();
    #[cfg(not(feature="disable_test_deadline"))]
    assert!(duration >= timeout && duration <= timeout + TIMEOUT_MARGIN,
        "blocking time incorrect: {:?}, wanted: >= {:?} and >= {:?}.", duration, timeout, timeout + TIMEOUT_MARGIN);

    let start = Instant::now();
    poll::<_, ()>(&mut [&mut SleepySource, &mut AvailableSource(Duration::from_secs(1))], &mut events, Some(timeout)).unwrap();
    assert!(events.is_empty());
    let duration = start.elapsed();
    #[cfg(not(feature="disable_test_deadline"))]
    assert!(duration >= timeout && duration <= timeout + TIMEOUT_MARGIN,
        "blocking time incorrect: {:?}, wanted: >= {:?} and >= {:?}.", duration, timeout, timeout + TIMEOUT_MARGIN);
}

struct ResultSource<E>(Result<(), E>);

impl<E2, ES, E> event::Source<ES, E> for ResultSource<E2>
    where ES: event::Sink,
          E: From<E2>,
          E2: Clone,
{
    fn next_event_available(&self) -> Option<Duration> {
        None
    }

    fn poll(&mut self, _event_sink: &mut ES) -> Result<(), E> {
        self.0.clone().map_err(Into::into)
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Error {
    U8(u8),
    U16(u16),
    U32(u32),
}

impl From<u8> for Error {
    fn from(err: u8) -> Self {
        Error::U8(err)
    }
}

impl From<u16> for Error {
    fn from(err: u16) -> Self {
        Error::U16(err)
    }
}

impl From<u32> for Error {
    fn from(err: u32) -> Self {
        Error::U32(err)
    }
}

#[test]
fn poll_different_source_error_types() {
    init();

    let mut events = Vec::new();

    let mut s1 = ResultSource::<u8>(Ok(()));
    let mut s2 = ResultSource(Err(1u8));
    let mut s3 = ResultSource(Err(2u16));
    let mut s4 = ResultSource(Err(3u32));

    let res = poll(&mut [&mut s1, &mut s2, &mut s3, &mut s4], &mut events, None);
    assert_eq!(res, Err(Error::U8(1)));
}
