use std::{cmp, io, mem, ptr};
use std::os::unix::io::RawFd;
use std::time::Duration;

use libc;
use log::error;

use crate::event::{Event, EventedId, Events, Ready, INVALID_EVENTED_ID};
use crate::poll::PollOption;
use crate::sys::EVENTS_CAP;

// Of course each OS that implements kqueue has chosen to go for different types
// in the `kevent` structure, hence the type definitions below.

// Type of `nchanges` in the `kevent` system call.
#[cfg(not(target_os = "netbsd"))]
#[allow(non_camel_case_types)]
type nchanges_t = libc::c_int;
#[cfg(target_os = "netbsd")]
#[allow(non_camel_case_types)]
type nchanges_t = libc::size_t;

// Type of the `filter` field in the `kevent` structure.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
#[allow(non_camel_case_types)]
type kevent_filter_t = libc::c_short;
#[cfg(target_os = "macos")]
#[allow(non_camel_case_types)]
type kevent_filter_t = libc::int16_t;
#[cfg(target_os = "netbsd")]
#[allow(non_camel_case_types)]
type kevent_filter_t = libc::uint32_t;

// Type of the `flags` field in the `kevent` structure.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
#[allow(non_camel_case_types)]
type kevent_flags_t = libc::c_ushort;
#[cfg(target_os = "macos")]
#[allow(non_camel_case_types)]
type kevent_flags_t = libc::uint16_t;
#[cfg(target_os = "netbsd")]
#[allow(non_camel_case_types)]
type kevent_flags_t = libc::uint32_t;

// Type of the `data` field in the `kevent` structure.
#[cfg(any(target_os = "freebsd", target_os = "macos"))]
#[allow(non_camel_case_types)]
type kevent_data_t = libc::intptr_t;
#[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
#[allow(non_camel_case_types)]
type kevent_data_t = libc::int64_t;

// Type of the `udata` field in the `kevent` structure.
#[cfg(any(target_os = "freebsd", target_os = "macos", target_os = "openbsd"))]
#[allow(non_camel_case_types)]
type kevent_udata_t = *mut libc::c_void;
#[cfg(target_os = "netbsd")]
#[allow(non_camel_case_types)]
type kevent_udata_t = libc::intptr_t;

#[derive(Debug)]
pub struct Selector {
    kq: RawFd,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        let kq = unsafe { libc::kqueue() };
        if kq == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Selector { kq })
        }
    }

    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let mut kevents: [libc::kevent; EVENTS_CAP] = unsafe { mem::uninitialized() };
        let events_cap = cmp::min(events.capacity(), EVENTS_CAP) as nchanges_t;

        let timespec = timeout.map(timespec_from_duration);
        #[allow(trivial_casts)]
        let timespec_ptr = timespec
            .as_ref()
            .map(|t| t as *const libc::timespec)
            .unwrap_or(ptr::null());

        let n_events = unsafe {
            libc::kevent(self.kq, ptr::null(), 0,
                kevents.as_mut_ptr(), events_cap, timespec_ptr)
        };
        match n_events {
            -1 => {
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    // The call was interrupted, try again.
                    // FIXME: the timeout should be reduced here, since time has
                    // passed.
                    Some(libc::EINTR) => self.select(events, timeout),
                    _ => Err(err),
                }
            },
            0 => Ok(()), // Reached the time limit, no events are pulled.
            n => {
                for kevent in kevents.iter().take(n as usize) {
                    let event = kevent_to_event(kevent);
                    events.push(event);
                }
                Ok(())
            },
        }
    }

    pub fn register(&self, fd: RawFd, id: EventedId, interests: Ready, opt: PollOption) -> io::Result<()> {
        let flags = opt_to_flags(opt) | libc::EV_ADD;
        // At most we need two changes, but maybe we only need 1.
        let mut changes: [libc::kevent; 2] = unsafe { mem::uninitialized() };
        let mut n_changes = 0;

        if interests.contains(Ready::WRITABLE) {
            let kevent = new_kevent(fd as libc::uintptr_t, libc::EVFILT_WRITE, flags, id);
            unsafe { ptr::write(&mut changes[n_changes], kevent) };
            n_changes += 1;
        }

        if interests.contains(Ready::READABLE) {
            let kevent = new_kevent(fd as libc::uintptr_t, libc::EVFILT_READ, flags, id);
            unsafe { ptr::write(&mut changes[n_changes], kevent) };
            n_changes += 1;
        }

        kevent_register(self.kq, &mut changes[0..n_changes], &[])
    }

    pub fn reregister(&self, fd: RawFd, id: EventedId, interests: Ready, opt: PollOption) -> io::Result<()> {
        let flags = opt_to_flags(opt);
        let write_flags = if interests.is_writable() {
            flags | libc::EV_ADD
        } else {
            flags | libc::EV_DELETE
        };
        let read_flags = if interests.is_readable() {
            flags | libc::EV_ADD
        } else {
            flags | libc::EV_DELETE
        };

        let mut changes: [libc::kevent; 2] = [
            new_kevent(fd as libc::uintptr_t, libc::EVFILT_WRITE, write_flags, id),
            new_kevent(fd as libc::uintptr_t, libc::EVFILT_READ, read_flags, id),
        ];

        kevent_register(self.kq, &mut changes, &[libc::ENOENT as kevent_data_t])
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let flags = libc::EV_DELETE | libc::EV_RECEIPT;
        // id is not used.
        let mut changes: [libc::kevent; 2] = [
            new_kevent(fd as libc::uintptr_t, libc::EVFILT_WRITE, flags, INVALID_EVENTED_ID),
            new_kevent(fd as libc::uintptr_t, libc::EVFILT_READ, flags, INVALID_EVENTED_ID),
        ];

        kevent_register(self.kq, &mut changes, &[libc::ENOENT as kevent_data_t])
    }
}

/// Create a `timespec` from a duration.
fn timespec_from_duration(duration: Duration) -> libc::timespec {
    libc::timespec {
        tv_sec: cmp::min(duration.as_secs(), libc::time_t::max_value() as u64) as libc::time_t,
        tv_nsec: libc::c_long::from(duration.subsec_nanos()),
    }
}

/// Convert a `kevent` into an `Event`.
fn kevent_to_event(kevent: &libc::kevent) -> Event {
    let id = EventedId(kevent.udata as usize);
    let mut readiness = Ready::empty();

    if contains_flag(kevent.flags, libc::EV_ERROR)  {
        // The actual error is stored in `kevent.data`, but we can't pass it
        // to the user from here. So the user needs to try and retrieve the
        // error themselves.
        readiness.insert(Ready::ERROR);
    }

    if contains_flag(kevent.flags, libc::EV_EOF) {
        readiness.insert(Ready::HUP);

        // When the read end of the socket is closed, EV_EOF is set on
        // flags, and fflags contains the error if there is one.
        if kevent.fflags != 0 {
            readiness.insert(Ready::ERROR);
        }
    }

    if kevent.filter == libc::EVFILT_READ {
        readiness.insert(Ready::READABLE);
    }

    if kevent.filter == libc::EVFILT_WRITE {
        readiness.insert(Ready::WRITABLE);
    }

    Event::new(id, readiness)
}

/// Convert poll options into `kevent` flags.
fn opt_to_flags(opt: PollOption) -> kevent_flags_t {
    libc::EV_RECEIPT | match opt {
        PollOption::Edge => libc::EV_CLEAR,
        PollOption::Level => 0, // Default.
        PollOption::Oneshot => libc::EV_ONESHOT,
    }
}

/// Create a new `kevent`.
const fn new_kevent(ident: libc::uintptr_t, filter: kevent_filter_t, flags: kevent_flags_t, id: EventedId) -> libc::kevent {
    libc::kevent {
        ident, filter, flags,
        fflags: 0,
        data: 0,
        udata: id.0 as kevent_udata_t,
    }
}

fn kevent_register(kq: RawFd, changes: &mut [libc::kevent], ignored_errors: &[kevent_data_t]) -> io::Result<()> {
    // No blocking.
    let timeout = libc::timespec { tv_sec: 0, tv_nsec: 0 };

    let ok = unsafe {
        libc::kevent(kq, changes.as_ptr(), changes.len() as nchanges_t,
            changes.as_mut_ptr(), changes.len() as nchanges_t, &timeout)
    };

    if ok == -1 {
        // EINTR is the only error that we can handle, but according to the man
        // page of FreeBSD: "When kevent() call fails with EINTR error, all
        // changes in the changelist have been applied", so we're done.
        //
        // EOPNOTSUPP (NetBSD only),
        // EACCES, EEBADF, FAULT, EINVAL and ESRCH: all have to do with invalid
        //                                          argument, which shouldn't
        //                                          happen or are the users
        //                                          fault, e.g. bad fd.
        // ENOMEM: can't handle.
        // ENOENT: invalid argument, or in case of deregister will be set in the
        //         change and ignored.
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EINTR) => Ok(()),
            _ => Err(err),
        }
    } else {
        check_errors(&*changes, ignored_errors)
    }
}

/// Check all events for possible errors, it returns the first error found.
fn check_errors(events: &[libc::kevent], ignored_errors: &[kevent_data_t]) -> io::Result<()> {
    for event in &*events {
        // Check for the error flag, the actual error will be in the `data`
        // field.
        if contains_flag(event.flags, libc::EV_ERROR) && event.data != 0 &&
            // Make sure the error is not one to ignore.
            !ignored_errors.contains(&event.data)
        {
            return Err(io::Error::from_raw_os_error(event.data as i32));
        }
    }
    Ok(())
}

/// Whether or not the provided `flags` contains the provided `flag`.
const fn contains_flag(flags: kevent_flags_t, flag: kevent_flags_t) -> bool {
    (flags & flag) != 0
}

impl Drop for Selector {
    fn drop(&mut self) {
        if unsafe { libc::close(self.kq) } == -1 {
            // Possible errors:
            // - EBADF, EIO: can't recover.
            // - EINTR: could try again but we're can't be sure if the file
            //          descriptor was closed or not, so to be safe we don't
            //          close it again.
            let err = io::Error::last_os_error();
            error!("error closing kqueue: {}", err);
        }
    }
}
