use std::{io, mem, ptr};

use crate::os::signals::SignalSet;

#[cfg(target_os = "linux")]
mod signalfd {
    use std::fs::File;
    use std::io::{self, Read};
    use std::os::unix::io::FromRawFd;
    use std::{mem, slice};

    use super::{block_signals, create_sigset};
    use crate::event;
    use crate::os::signals::{Signal, SignalSet};
    use crate::os::{Interests, RegisterOption};
    use crate::sys::Selector;

    /// Signaler backed by `signalfd`.
    #[derive(Debug)]
    pub struct Signals {
        fd: File,
    }

    impl Signals {
        pub fn new(selector: &Selector, signals: SignalSet, id: event::Id) -> io::Result<Signals> {
            // Create a mask for all signal we want to handle.
            let set = create_sigset(signals)?;

            // Create a new signal file descriptor.
            let fd = unsafe { libc::signalfd(-1, &set, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK) };
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }

            // Register the signalfd, only then block the signals and return our
            // struct.
            selector.register(fd, id, Interests::READABLE, RegisterOption::LEVEL)
                .and_then(|()| block_signals(&set))
                .map(|()| Signals { fd: unsafe { File::from_raw_fd(fd) } })
        }

        pub fn receive(&mut self) -> io::Result<Option<Signal>> {
            let mut info: libc::signalfd_siginfo = unsafe { mem::uninitialized() };
            #[allow(trivial_casts)]
            let info_ref: &mut [u8] = unsafe { slice::from_raw_parts_mut(&mut info as *mut _ as *mut u8, mem::size_of::<libc::signalfd_siginfo>()) };
            let n = loop {
                match self.fd.read(info_ref) {
                    Ok(ok) => break ok,
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                    Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(err) => return Err(err),
                }
            };
            assert_eq!(n, mem::size_of::<libc::signalfd_siginfo>());
            Ok(Signal::from_raw(info.ssi_signo as libc::c_int))
        }
    }
}

#[cfg(target_os = "linux")]
pub use self::signalfd::Signals;

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
mod kqueue {
    use std::os::unix::io::AsRawFd;
    use std::{io, mem, ptr};

    use super::{block_signals, create_sigset};
    use crate::event;
    use crate::os::signals::{Signal, SignalSet};
    use crate::os::{Interests, RegisterOption};
    use crate::sys::Selector;

    /// Signaler backed by kqueue (`EVFILT_SIGNAL`).
    #[derive(Debug)]
    pub struct Signals {
        kq: Selector,
    }

    impl Signals {
        pub fn new(selector: &Selector, signals: SignalSet, id: event::Id) -> io::Result<Signals> {
            let set = create_sigset(signals)?;
            let kq = Selector::new()?;
            kq.register_signals(id, signals)
                .and_then(|()| selector.register(kq.as_raw_fd(), id,
                    Interests::READABLE, RegisterOption::LEVEL))
                .and_then(|()| block_signals(&set))
                .map(|()| Signals { kq })
        }

        pub fn receive(&mut self) -> io::Result<Option<Signal>> {
            let mut kevent: libc::kevent = unsafe { mem::uninitialized() };
            let timeout = libc::timespec { tv_sec: 0, tv_nsec: 0 };

            let n_events = unsafe {
                libc::kevent(self.kq.as_raw_fd(), ptr::null(), 0,
                    &mut kevent, 1, &timeout)
            };
            match n_events {
                -1 => Err(io::Error::last_os_error()),
                0 => Ok(None), // No signals.
                n => {
                    assert_eq!(n, 1);
                    let filter = kevent.filter;
                    assert_eq!(filter, libc::EVFILT_SIGNAL);
                    Ok(Signal::from_raw(kevent.ident as libc::c_int))
                },
            }
        }
    }
}

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
pub use self::kqueue::Signals;

/// Create a `libc::sigset_t` from `SignalSet`.
fn create_sigset(signals: SignalSet) -> io::Result<libc::sigset_t> {
    let mut set: libc::sigset_t = unsafe { mem::uninitialized() };
    if unsafe { libc::sigemptyset(&mut set) } == -1 {
        return Err(io::Error::last_os_error());
    }
    for signal in signals {
        if unsafe { libc::sigaddset(&mut set, signal.into_raw()) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(set)
}

/// Block all signals in `set`.
fn block_signals(set: &libc::sigset_t) -> io::Result<()> {
    if unsafe { libc::sigprocmask(libc::SIG_BLOCK, set, ptr::null_mut()) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
