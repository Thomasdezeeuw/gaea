# Gaea

[![Build Status](https://travis-ci.org/Thomasdezeeuw/gaea.svg?branch=master)](https://travis-ci.org/Thomasdezeeuw/gaea)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/gaea.svg)](https://crates.io/crates/gaea)
[![Docs](https://docs.rs/gaea/badge.svg)](https://docs.rs/gaea)

Low-level library to build event driven applications, supporting lightweight
non-blocking I/O.

This crate started as a fork of [mio] (v0.6.12, commit
4a716d0b687592368d9e283a6ea63aedb5877fc8), changed to run on a single thread.
But since the has evolved dramatically changing to become the center of events,
rather then just provided a cross-platform epoll/kqueue implementation.

[mio]: https://github.com/carllerche/mio

Rust version 1.33 or higher is required as gaea makes use of Rust 2018 edition
features.


# Deprecation notice

Gaea is deprecated in favour of [Mio] as I've joined the Mio team and will
continue developing the Mio crate instead of Gaea.

[Mio]: https://crates.io/crates/mio


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

[API documentation]: https://docs.rs/gaea


## License

Licensed under the MIT license ([LICENSE] or
https://opensource.org/licenses/MIT).

[LICENSE]: ./LICENSE


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be licensed as above, without any
additional terms or conditions.
