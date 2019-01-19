#[cfg(target_os = "linux")]
mod eventfd {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::mem;
    use std::os::unix::io::FromRawFd;

    use libc;

    use crate::event::EventedId;
    use crate::poll::{Interests, PollOption};
    use crate::sys::Selector;

    /// Awakener backed by `eventfd`.
    ///
    /// `eventfd` is effectively an 64 bit counter. All writes must be of 8
    /// bytes (64 bits) and are converted (native endian) into an 64 bit
    /// unsigned integer and added to the count. Reads must also be 8 bytes and
    /// reset the count to 0, returning the count.
    #[derive(Debug)]
    pub struct Awakener {
        fd: File,
    }

    impl Awakener {
        pub fn new(selector: &Selector, id: EventedId) -> io::Result<Awakener> {
            let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }

            selector.register(fd, id, Interests::READABLE, PollOption::Edge)?;
            Ok(Awakener {
                fd: unsafe { File::from_raw_fd(fd) },
            })
        }

        pub fn try_clone(&self) -> io::Result<Awakener> {
            Ok(Awakener {
                fd: self.fd.try_clone()?,
            })
        }

        pub fn wake(&self) -> io::Result<()> {
            let buf: [u8; 8] = unsafe { mem::transmute(1u64) };
            (&self.fd).write(&buf).map(|_| ())
        }

        pub fn drain(&self) -> io::Result<()> {
            let mut buf: [u8; 8] = [0; 8];
            match (&self.fd).read(&mut buf) {
                Ok(_) => Ok(()),
                // If the `Awakener` hasn't been awoken yet this will return a
                // `WouldBlock` error which we can safely ignore.
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => Ok(()),
                Err(err) => Err(err),
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use self::eventfd::Awakener;

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
mod kqueue {
    use std::io;

    use crate::event::EventedId;
    use crate::sys::Selector;

    /// Awakener backed by kqueue user space notifications (`EVFILT_USER`).
    ///
    /// The implementation is fairly simple, first the kqueue must be setup to
    /// receive awakener events this done by calling `Selector.setup_awakener`.
    /// Next we need access to kqueue, thus we need to duplicate the file
    /// descriptor. Now waking is as simple as adding an event to the kqueue.
    #[derive(Debug)]
    pub struct Awakener {
        selector: Selector,
        id: EventedId,
    }

    impl Awakener {
        pub fn new(selector: &Selector, id: EventedId) -> io::Result<Awakener> {
            selector.setup_awakener(id)?;
            Ok(Awakener {
                selector: selector.try_clone()?,
                id,
            })
        }

        pub fn try_clone(&self) -> io::Result<Awakener> {
            Ok(Awakener {
                selector: self.selector.try_clone()?,
                id: self.id,
            })
        }

        pub fn wake(&self) -> io::Result<()> {
            self.selector.awake(self.id)
        }

        pub fn drain(&self) -> io::Result<()> {
            // Nothing needs to be done.
            Ok(())
        }
    }
}

#[cfg(any(target_os = "freebsd", target_os = "macos",
          target_os = "netbsd", target_os = "openbsd"))]
pub use self::kqueue::Awakener;
