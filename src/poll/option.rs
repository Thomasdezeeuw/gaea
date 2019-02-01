/// Option supplied when [registering] an `Evented` handle with `Poller`.
///
/// `PollOption` values can be combined together using the various bitwise
/// operators.
///
/// For high level documentation on polling see [`Poller`].
///
/// [registering]: struct.Poller.html#method.register
/// [`Poller`]: struct.Poller.html
///
/// # Difference
///
/// An [`Evented`] registration may request [edge-triggered] or
/// [level-triggered] or [oneshot] events. `Evented` handle registered with
/// oneshot polling option will only receive a single event and then have to
/// [reregister] too receive more events.
///
/// The difference between the edge-triggered and level-trigger can be described
/// as follows. Supposed that this scenario happens:
///
/// 1. A [`TcpStream`] is registered with `Poller`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poller.poll`] returns the id associated with the socket
///    indicating readable readiness.
/// 4. 1kb is read from the socket.
/// 5. Another call to [`Poller.poll`] is made.
///
/// If when the socket was registered with `Poller`, and *edge*-triggered events
/// were requested, then the call to [`Poller.poll`] done in step **5** will
/// (probably) block despite there being another 1kb still present in the socket
/// read buffer. The reason for this is that edge-triggered mode delivers events
/// only when changes occur on the monitored [`Evented`]. So, in step *5* the
/// caller might end up waiting for some data that is already present inside the
/// socket buffer.
///
/// With edge-triggered events, operations **must** be performed on the
/// `Evented` type until [`WouldBlock`] is returned. In other words, after
/// receiving an event indicating readiness for a certain operation, one should
/// assume that [`Poller.poll`] may never return another event for the same id
/// and readiness until the operation returns [`WouldBlock`].
///
/// By contrast, when *level*-triggered notifications was requested, each call
/// to [`Poller.poll`] will return an event for the socket as long as data
/// remains in the socket buffer. Though generally, level-triggered events
/// should be avoided if high performance is a concern.
///
/// Since even with edge-triggered events, multiple events can be generated upon
/// receipt of multiple chunks of data, the caller has the option to set the
/// oneshot flag. This tells `Poller` to disable the associated [`Evented`]
/// after the event is returned from [`Poller.poll`], note that *disabled* and
/// *deregistered* are not the same thing. Subsequent calls to [`Poller.poll`]
/// will no longer include events for [`Evented`] handles that are disabled even
/// if the readiness state changes. The handle can be re-enabled by calling
/// [`reregister`]. When handles are disabled, internal resources used to
/// monitor the handle are maintained until the handle is deregistered. This
/// makes re-registering the handle a fast operation.
///
/// For example, in the following scenario:
///
/// 1. A [`TcpStream`] is registered with `Poller`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poller.poll`] returns the id associated with the socket
///    indicating readable readiness.
/// 4. 2kb is read from the socket.
/// 5. Another call to read is issued and [`WouldBlock`] is returned
/// 6. The socket receives another 2kb of data.
/// 7. Another call to [`Poller.poll`] is made.
///
/// Assuming the socket was registered with `Poller` with the *oneshot* option,
/// then the call to [`Poller.poll`] in step 7 would block. This is because,
/// oneshot tells `Poller` to disable events for the socket after returning an
/// event.
///
/// In order to receive the event for the data received in step 6, the socket
/// would need to be reregistered using [`reregister`].
///
/// [`Evented`]: ../event/trait.Evented.html
/// [edge-triggered]: #variant.Edge
/// [level-triggered]: #variant.Level
/// [oneshot]: #variant.Oneshot
/// [reregister]: struct.Poller.html#method.reregister
/// [`TcpStream`]: ../net/struct.TcpStream.html
/// [`Poller.poll`]: struct.Poller.html#method.poll
/// [`WouldBlock`]: https://doc.rust-lang.org/nightly/std/io/enum.ErrorKind.html#variant.WouldBlock
/// [`reregister`]: struct.Poller.html#method.reregister
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PollOption {
    /// Edge-triggered notifications.
    Edge,
    /// Level-triggered notifications.
    Level,
    /// Oneshot notifications.
    Oneshot,
}
