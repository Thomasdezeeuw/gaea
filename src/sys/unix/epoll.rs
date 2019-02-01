use std::{cmp, io, mem, ptr};
use std::os::unix::io::RawFd;
use std::time::Duration;

use libc;
use log::error;

use crate::event::{Event, EventedId, Events, Ready};
use crate::poll::{Interests, PollOption};
use crate::sys::EVENTS_CAP;

#[derive(Debug)]
pub struct Selector {
    epfd: RawFd,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        let epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epfd == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Selector { epfd })
        }
    }

    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let mut ep_events: [libc::epoll_event; EVENTS_CAP] = unsafe { mem::uninitialized() };
        let events_cap = cmp::min(events.capacity(), EVENTS_CAP) as libc::c_int;

        let timeout_ms = timeout.map(duration_to_millis).unwrap_or(-1);

        let n_events = unsafe {
            libc::epoll_wait(self.epfd, ep_events.as_mut_ptr(), events_cap, timeout_ms)
        };
        match n_events {
            -1 => Err(io::Error::last_os_error()),
            0 => Ok(()), // Reached the time limit, no events are pulled.
            n => {
                for ep_event in ep_events.iter().take(n as usize) {
                    let event = ep_event_to_event(ep_event);
                    events.push(event);
                }
                Ok(())
            },
        }
    }

    pub fn register(&self, fd: RawFd, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        let mut epoll_event = new_epoll_event(interests, opt, id);
        epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut epoll_event)
    }

    pub fn reregister(&self, fd: RawFd, id: EventedId, interests: Interests, opt: PollOption) -> io::Result<()> {
        let mut epoll_event = new_epoll_event(interests, opt, id);
        epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut epoll_event)
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut())
    }
}

const MILLIS_PER_SEC: u64 = 1_000;
const NANOS_PER_MILLI: u64 = 1_000_000;

/// Convert a `Duration` to milliseconds.
pub fn duration_to_millis(duration: Duration) -> libc::c_int {
    let millis = duration.as_secs().saturating_mul(MILLIS_PER_SEC)
        .saturating_add((duration.subsec_nanos() as u64 / NANOS_PER_MILLI) + 1);
    cmp::min(millis, libc::c_int::max_value() as u64) as libc::c_int
}

/// Convert a `epoll_event` into an `Event`.
fn ep_event_to_event(ep_event: &libc::epoll_event) -> Event {
    let id = EventedId(ep_event.u64 as usize);
    let epoll = ep_event.events;
    let mut readiness = Ready::empty();

    if contains_flag(epoll, libc::EPOLLIN | libc::EPOLLPRI) {
        readiness.insert(Ready::READABLE);
    }

    if contains_flag(epoll, libc::EPOLLOUT) {
        readiness.insert(Ready::WRITABLE);
    }

    if contains_flag(epoll, libc::EPOLLERR) {
        readiness.insert(Ready::ERROR);
    }

    if contains_flag(epoll, libc::EPOLLRDHUP | libc::EPOLLHUP) {
        readiness.insert(Ready::HUP);
    }

    Event::new(id, readiness)
}

/// Whether or not the provided `flags` contains the provided `flag`.
fn contains_flag(flags: libc::uint32_t, flag: libc::c_int) -> bool {
    (flags & flag as libc::uint32_t) != 0
}

/// Create a new `epoll_event`.
fn new_epoll_event(interests: Interests, opt: PollOption, id: EventedId) -> libc::epoll_event {
    libc::epoll_event {
        events: to_epoll_events(interests, opt),
        u64: id.0 as u64,
    }
}

fn to_epoll_events(interests: Interests, opt: PollOption) -> libc::uint32_t {
    let mut events = libc::EPOLLPRI | libc::EPOLLRDHUP;

    if interests.is_readable() {
        events |= libc::EPOLLIN;
    }

    if interests.is_writable() {
        events |= libc::EPOLLOUT;
    }

    events |= match opt {
        PollOption::Edge => libc::EPOLLET,
        PollOption::Level => 0, // Default.
        PollOption::Oneshot => libc::EPOLLONESHOT,
    };
    events as libc::uint32_t
}

fn epoll_ctl(epfd: RawFd, op: libc::c_int, fd: RawFd, event: *mut libc::epoll_event) -> io::Result<()> {
    if unsafe { libc::epoll_ctl(epfd, op, fd, event) } == -1 {
        // Possible errors:
        // EBADF, EEXIST, ENOENT, EPERM: user error.
        // EINVAL, ELOOP: shouldn't happen.
        // ENOMEM, ENOSPC: can't handle.
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        if unsafe { libc::close(self.epfd) } == -1 {
            // Possible errors:
            // - EBADF, EIO: can't recover.
            // - EINTR: could try again but we're can't be sure if the file
            //          descriptor was closed or not, so to be safe we don't
            //          close it again.
            let err = io::Error::last_os_error();
            error!("error closing epoll: {}", err);
        }
    }
}
