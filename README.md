# Mio-st - Metal IO, single threaded

[![Build Status](https://travis-ci.org/Thomasdezeeuw/mio-st.svg?branch=master)](https://travis-ci.org/Thomasdezeeuw/mio-st)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mio_st.svg)](https://crates.io/crates/mio-st)
[![Docs](https://docs.rs/mio-st/badge.svg)](https://docs.rs/mio-st)

This is a fork of [mio] (v0.6.12, commit
4a716d0b687592368d9e283a6ea63aedb5877fc8), changed to run on a single thread.

[mio]: https://github.com/carllerche/mio

Rust version 1.33 or higher is required as mio-st makes use of Rust 2018 edition
features.


## Differences compared to mio

The main two differences compared to [mio] are:
 - Focus on single threaded performance.
 - No Windows support.

The goal of this crate was to reduce the overhead of locks and/or atomic
operations, at the cost of dropping the multi-threaded user queue. This means
the usage of this crates, compared to mio, changes to using a single `OsQueue`
(`Poll` in mio) per thread. Where when using mio you might use a single `Poll`
instance for the entire application.

When reworking the code Windows support was removed because the underlying
polling technique provided by the OS differs too much from epoll and kqueue.
Carl Lerche (@carllerche, the auther of mio) did an amazing job of supporting
Windows, but I have no interest in supporting Windows (I simply don't use it).


## OS support

The following platforms are supported:

 - Linux (production target), and
 - macOS (development target).

The following platforms should work, as in the code compiles:

 - FreeBSD,
 - NetBSD, and
 - OpenBSD.


## Documentation

The [API documentation] is available on docs.rs.

[API documentation]: https://docs.rs/mio-st


## License

Licensed under the MIT license ([LICENSE] or
https://opensource.org/licenses/MIT).

[LICENSE]: ./LICENSE


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be licensed as above, without any
additional terms or conditions.
