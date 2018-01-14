pub use self::pipe::Awakener;

/// Default awakener backed by a pipe
mod pipe {
    use std::io::{self, Read, Write};
    use std::os::unix::io::AsRawFd;

    use sys::unix::{self, Selector};
    use {Ready, Poll, PollOpt, Token};
    use event::Evented;

    /*
     *
     * ===== Awakener =====
     *
     */

    pub struct Awakener {
        reader: unix::Io,
        writer: unix::Io,
    }

    impl Awakener {
        pub fn new() -> io::Result<Awakener> {
            let (rd, wr) = unix::pipe()?;

            Ok(Awakener {
                reader: rd,
                writer: wr,
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

        fn reader(&mut self) -> &mut unix::Io {
            &mut self.reader
        }
    }

    impl Evented for Awakener {
        fn register(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
            self.reader().register(poll, token, interest, opts)
        }

        fn reregister(&mut self, poll: &mut Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
            self.reader().reregister(poll, token, interest, opts)
        }

        fn deregister(&mut self, poll: &mut Poll) -> io::Result<()> {
            self.reader().deregister(poll)
        }
    }
}
