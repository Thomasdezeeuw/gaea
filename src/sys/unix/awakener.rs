use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};

use nix;
use nix::fcntl::{fcntl, FcntlArg, O_CLOEXEC, O_NONBLOCK};

use sys::unix::{nix_to_io_error, Selector, Io};
use event::Evented;
use poll::{Poll, PollOpt, Ready, Token};

pub struct Awakener {
    reader: Io,
    writer: Io,
}

impl Awakener {
    pub fn new() -> io::Result<Awakener> {
        let (r_fd, w_fd) = nix::unistd::pipe().map_err(nix_to_io_error)?;
        fcntl(r_fd, FcntlArg::F_SETFL(O_NONBLOCK | O_CLOEXEC))
            .map_err(nix_to_io_error)?;
        fcntl(w_fd, FcntlArg::F_SETFL(O_NONBLOCK | O_CLOEXEC))
            .map_err(nix_to_io_error)?;

        Ok(Awakener {
            reader: unsafe { Io::from_raw_fd(r_fd) },
            writer: unsafe { Io::from_raw_fd(w_fd) },
        })
    }

    pub fn init(&mut self, selector: &mut Selector, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        selector.register(self.reader.as_raw_fd(), token, interest, opts)
    }

    pub fn wakeup(&self) -> io::Result<()> {
        match (&self.writer).write(&[1]) {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn cleanup(&self) {
        let mut buf = [0; 128];

        loop {
            // Consume data until all bytes are purged
            match (&self.reader).read(&mut buf) {
                Ok(i) if i > 0 => {},
                _ => return,
            }
        }
    }
}

impl Evented for Awakener {
    fn register(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.reader.register(poll, token, interest, opts)
    }

    fn reregister(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.reader.reregister(poll, token, interest, opts)
    }

    fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
        self.reader.deregister(poll)
    }
}
