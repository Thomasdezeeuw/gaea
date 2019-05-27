#[cfg(any(target_os = "linux", target_os = "android"))]
mod eventfd {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::mem;
    use std::os::unix::io::FromRawFd;

    use crate::event;
    use crate::os::{Interests, RegisterOption};
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
        pub fn new(selector: &Selector, id: event::Id) -> io::Result<Awakener> {
            let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }

            selector.register(fd, id, Interests::READABLE, RegisterOption::EDGE)?;
            Ok(Awakener {
                fd: unsafe { File::from_raw_fd(fd) },
            })
        }

        pub fn try_clone(&self) -> io::Result<Awakener> {
            self.fd.try_clone().map(|fd| Awakener { fd })
        }

        pub fn wake(&self) -> io::Result<()> {
            let buf: [u8; 8] = unsafe { mem::transmute(1u64) };
            match (&self.fd).write(&buf) {
                Ok(_) => Ok(()),
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    // Writing only blocks if the counter is going to overflow.
                    // So we'll reset the counter to 0 and wake it again.
                    self.reset()?;
                    self.wake()
                },
                Err(err) => Err(err),
            }
        }

        /// Reset the eventfd object, only need to call this if `wake` fails.
        fn reset(&self) -> io::Result<()> {
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

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::eventfd::Awakener;

#[cfg(any(target_os = "freebsd", target_os = "macos"))]
mod kqueue {
    use std::io;

    use crate::event;
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
        id: event::Id,
    }

    impl Awakener {
        pub fn new(selector: &Selector, id: event::Id) -> io::Result<Awakener> {
            selector.try_clone().and_then(|selector| {
                selector.setup_awakener(id)
                    .map(|()| Awakener { selector, id })
            })
        }

        pub fn try_clone(&self) -> io::Result<Awakener> {
            self.selector.try_clone().map(|selector| Awakener {
                selector,
                id: self.id,
            })
        }

        pub fn wake(&self) -> io::Result<()> {
            self.selector.wake(self.id)
        }
    }
}

#[cfg(any(target_os = "freebsd", target_os = "macos"))]
pub use self::kqueue::Awakener;

#[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
mod pipe {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

    use crate::event;
    use crate::os::{Interests, RegisterOption};
    use crate::sys::Selector;
    use crate::unix::new_pipe;

    /// Awakener backed by a unix pipe.
    ///
    /// Awakener controls both the sending and receiving ends and empties the
    /// pipe if writing to it (waking) fails.
    #[derive(Debug)]
    pub struct Awakener {
        sender: File,
        receiver: File,
    }

    impl Awakener {
        pub fn new(selector: &Selector, id: event::Id) -> io::Result<Awakener> {
            let (sender, receiver) = new_pipe()?;
            selector.register(receiver.as_raw_fd(), id, Interests::READABLE, RegisterOption::EDGE)?;
            Ok(Awakener {
                sender: unsafe { File::from_raw_fd(sender.into_raw_fd()) },
                receiver: unsafe { File::from_raw_fd(receiver.into_raw_fd()) },
            })
        }

        pub fn try_clone(&self) -> io::Result<Awakener> {
            Ok(Awakener {
                sender: self.sender.try_clone()?,
                receiver: self.receiver.try_clone()?,
            })
        }

        pub fn wake(&self) -> io::Result<()> {
            match (&self.sender).write(&[1]) {
                Ok(_) => Ok(()),
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    // The reading end is full so we'll empty the buffer and try
                    // again.
                    self.empty();
                    self.wake()
                },
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => self.wake(),
                Err(err) => Err(err)
            }
        }

        /// Empty the pipe's buffer, only need to call this if `wake` fails.
        /// This ignores any errors.
        fn empty(&self)  {
            let mut buf = [0; 4096];
            loop {
                match (&self.receiver).read(&mut buf) {
                    Ok(n) if n > 0 => continue,
                    _ => return,
                }
            }
        }
    }
}

#[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
pub use self::pipe::Awakener;
