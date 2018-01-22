/// Options supplied when registering an `Evented` handle with `Poll`.
///
/// `PollOpt` values can be combined together using the various bitwise
/// operators.
///
/// For high level documentation on polling see [`Poll`].
///
/// [`Poll`]: struct.Poll.html
///
/// # Edge-triggered and level-triggered
///
/// An [`Evented`] registration may request edge-triggered events or
/// level-triggered events. This is done by setting `register`'s [`PollOpt`]
/// argument to either [`edge`] or [`level`].
///
/// The difference between the two can be described as follows. Supposed that
/// this scenario happens:
///
/// 1. A [`TcpStream`] is registered with `Poll`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poll.poll`] returns the id associated with the socket
///    indicating readable readiness.
/// 4. 1kb is read from the socket.
/// 5. Another call to [`Poll.poll`] is made.
///
/// If when the socket was registered with `Poll`, and `edge` triggered events
/// were requested, then the call to [`Poll.poll`] done in step **5** will
/// (probably) block despite there being another 1kb still present in the socket
/// read buffer. The reason for this is that `edge`-triggered mode delivers
/// events only when changes occur on the monitored [`Evented`]. So, in step *5*
/// the caller might end up waiting for some data that is already present inside
/// the socket buffer.
///
/// With `edge`-triggered events, operations **must** be performed on the
/// `Evented` type until [`WouldBlock`] is returned. In other words, after
/// receiving an event indicating readiness for a certain operation, one should
/// assume that [`Poll.poll`] may never return another event for the same id and
/// readiness until the operation returns [`WouldBlock`].
///
/// By contrast, when `level`-triggered notifications was requested, each call
/// to [`Poll.poll`] will return an event for the socket as long as data remains
/// in the socket buffer. Though generally, `level`-triggered events should be
/// avoided if high performance is a concern.
///
/// Since even with `edge`-triggered events, multiple events can be generated
/// upon receipt of multiple chunks of data, the caller has the option to set
/// the [`oneshot`] flag. This tells `Poll` to disable the associated
/// [`Evented`] after the event is returned from [`Poll.poll`]. The subsequent
/// calls to [`Poll.poll`] will no longer include events for [`Evented`] handles
/// that are disabled even if the readiness state changes. The handle can be
/// re-enabled by calling [`reregister`]. When handles are disabled, internal
/// resources used to monitor the handle are maintained until the handle is
/// dropped or deregistered. This makes re-registering the handle a fast
/// operation.
///
/// For example, in the following scenario:
///
/// 1. A [`TcpStream`] is registered with `Poll`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poll.poll`] returns the id associated with the socket
///    indicating readable readiness.
/// 4. 2kb is read from the socket.
/// 5. Another call to read is issued and [`WouldBlock`] is returned
/// 6. The socket receives another 2kb of data.
/// 7. Another call to [`Poll.poll`] is made.
///
/// Assuming the socket was registered with `Poll` with the [`edge`] and
/// [`oneshot`] options, then the call to [`Poll.poll`] in step 7 would block.
/// This is because, [`oneshot`] tells `Poll` to disable events for the socket
/// after returning an event.
///
/// In order to receive the event for the data received in step 6, the socket
/// would need to be reregistered using [`reregister`].
///
/// [`Evented`]: ../event/trait.Evented.html
/// [`PollOpt`]: struct.PollOpt.html
/// [`edge`]: #associatedconstant.EDGE
/// [`level`]: #associatedconstant.LEVEL
/// [`TcpStream`]: ../net/struct.TcpStream.html
/// [`Poll.poll`]: struct.Poll.html#method.poll
/// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock
/// [`oneshot`]: #associatedconstant.ONESHOT
/// [`reregister`]: struct.Poll.html#method.reregister
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PollOpt {
    /// Edge-triggered notifications.
    Edge,
    /// Level-triggered notifications.
    Level,
    /// Oneshot notifications.
    Oneshot,
}
