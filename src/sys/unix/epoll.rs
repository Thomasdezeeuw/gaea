use std::cmp::min;
use std::os::unix::io::RawFd;
use std::time::Duration;
use std::{io, mem, ptr};

use log::error;

use crate::event::{self, Event, Ready};
use crate::os::{Interests, RegisterOption};
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

    pub fn select<ES>(&self, event_sink: &mut ES, timeout: Option<Duration>) -> io::Result<()>
        where ES: event::Sink,
    {
        let mut ep_events: [libc::epoll_event; EVENTS_CAP] = unsafe { mem::uninitialized() };
        let events_cap = event_sink.capacity_left().min(EVENTS_CAP) as libc::c_int;
        if events_cap == 0 {
            // epoll can't deal with 0 capacity event arrays.
            return Ok(())
        }

        let timeout_ms = timeout.map(duration_to_millis).unwrap_or(-1);

        let n_events = unsafe {
            libc::epoll_wait(self.epfd, ep_events.as_mut_ptr(), events_cap, timeout_ms)
        };
        match n_events {
            -1 => Err(io::Error::last_os_error()),
            0 => Ok(()), // Reached the time limit, no events are pulled.
            n => {
                let ep_events = ep_events[..n as usize].iter()
                    .map(ep_event_to_event);
                event_sink.extend(ep_events);
                Ok(())
            },
        }
    }

    pub fn register(&self, fd: RawFd, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        let mut epoll_event = new_epoll_event(interests, opt, id);
        epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut epoll_event)
    }

    pub fn reregister(&self, fd: RawFd, id: event::Id, interests: Interests, opt: RegisterOption) -> io::Result<()> {
        let mut epoll_event = new_epoll_event(interests, opt, id);
        epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut epoll_event)
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut())
    }
}

/// Convert a `Duration` to milliseconds.
///
/// # Notes
///
/// Uses 24 hours as maximum to match kqueue.
pub fn duration_to_millis(duration: Duration) -> libc::c_int {
    min(duration.as_millis(), 24 * 60 * 60 * 1_000) as libc::c_int
}

/// Convert a `epoll_event` into an `Event`.
fn ep_event_to_event(ep_event: &libc::epoll_event) -> Event {
    let id = event::Id(ep_event.u64 as usize);
    let epoll = ep_event.events;
    let mut readiness = Ready::EMPTY;

    if contains_flag(epoll, libc::EPOLLIN | libc::EPOLLPRI) {
        readiness |= Ready::READABLE;
    }

    if contains_flag(epoll, libc::EPOLLOUT) {
        readiness |= Ready::WRITABLE;
    }

    if contains_flag(epoll, libc::EPOLLERR) {
        readiness |= Ready::ERROR;
    }

    if contains_flag(epoll, libc::EPOLLRDHUP | libc::EPOLLHUP) {
        readiness |= Ready::HUP;
    }

    Event::new(id, readiness)
}

/// Whether or not the provided `flags` contains the provided `flag`.
const fn contains_flag(flags: u32, flag: libc::c_int) -> bool {
    (flags & flag as u32) != 0
}

/// Create a new `epoll_event`.
fn new_epoll_event(interests: Interests, opt: RegisterOption, id: event::Id) -> libc::epoll_event {
    libc::epoll_event {
        events: to_epoll_events(interests, opt),
        u64: id.0 as u64,
    }
}

fn to_epoll_events(interests: Interests, opt: RegisterOption) -> u32 {
    let mut events = libc::EPOLLPRI | libc::EPOLLRDHUP;

    if interests.is_readable() {
        events |= libc::EPOLLIN;
    }

    if interests.is_writable() {
        events |= libc::EPOLLOUT;
    }

    // NOTE: level is the default.
    if opt.is_edge() {
        events |= libc::EPOLLET;
    }
    if opt.is_oneshot() {
        events |= libc::EPOLLONESHOT;
    }
    events as u32
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
