# Changelog

## v0.3.0

The crate was renamed to Gaea, which comes with a complete redesign of the
crate's API.

### New

 * Now supports OpenBSD and NetBSD (properly).
 * Supports `no_std` environments.
 * New `event::Sink` trait that allows for a custom containers for events.
   `Events` was removed.
 * New `event::Source` trait to allow polling for events from different event
   sources.
 * `Queue`, an `event::Source` for user space queue for events.
 * `Timers`, an `event::Source` for deadlines and timeouts.
 * `poll` polls one or more event sources.
 * Add `os::Signals`, allows for handling of Unix signals.
 * Support for vectored I/O, when the "nightly" feature is enabled.

### Renamed/changed

 * Renamed `EventedId` to `event::Id` (in the `event` module).
 * Renamed `Poller` to `OsQueue` and moved it to a new `os` module.
 * Renamed `PollOption` to `RegisterOption` and moved it to the `os` module.
 * Moved `Evented` to the `os` module.
 * Moved `Awakener` to the `os` module.

### Removed

 * `EventedIo`, use `EventedFd` instead.
 * `ConnectedUdpSocket`, use `UdpSocket` instead.
 * `Events`, use `event::Sink` instead, for example with `Vec<Event>`.

## v0.2.3

### Changes

 * Fix kqueue backed Awakener, previously it would always trigger an event when
   polled.
 * **BREAKING** Remove `Awakener.drain`, `Awakener` automatically drains itself
   now.

## v0.2.2

### Changes

 * Minimum Rust version is now 1.31.
 * New `Awakener` type, used to awaken the poller from another thread.
 * `EPOLLPRI` and `EPOLLRDHUP` are now set by default for epoll.
 * `Ready::Readable` is not set when `EPOLLPRI` is received.
 * Derived `Hash` for `PollOption` and `Event`.

## v0.2.1

### Changes

 * `SO_REUSEPORT` and `SO_REUSEADDR` options are now set on `TcpListener`.

## v0.2.0

### New features:

 * Re-exported common types in the root of the crate.
 * Documented `Evented` handles that don't need to be deregistered.
 * Added new `Interests` type.
 * Added `INTERETS` associated constants to most `Evented` handles, for easy use
   in registering.

### **Breaking** changes:

 * Removed `Into<std::net::TcpStream>` implementation from `TcpStream`.
 * Manually implemented `Ready` type, dropping the `bitflags` dependency, also
   dropping some methods in the process.
 * Replaced `Ready` argument with `Interests` when registering `Evented` handles
   with `Poller` and in the `Evented` trait.
 * Removed `PollCalled` type from `Evented` trait.
 * Removed return argument from `Poller.notify`.
 * Removed return argument from `Poller.add_deadline` and `Poller.remove_deadline`.
 * All `EventedId` values are now valid.
 * Swap around `sys::unix::new_pipe` returned types to match `mspc` module in
   the standard library.
 * Removed the following `TcpStream` methods:
    - `set_keepalive`,
    - `keepalive`,
    - `connect_stream` and
    - `from_std_stream`.
 * Removed the following `TcpSListener` methods:
    - `accept_std` and
    - `from_std_listener`.
 * Removed the `UdpSocket.from_std_socket` method.
 * Removed the `ConnectedUdpSocket.from_connected_std_socket` method.
 * Updated code to Rust 2018 edition.

### Other changes:

 * Expanded testing of all types.
 * Dropped `net2` dependency.
 * Cleaned up logging.
 * Various improvements of documentation in the entire crate.

## v0.1.0

Initial release.
