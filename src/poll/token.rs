/// Associates readiness notifications with [`Evented`] handles.
///
/// `Token` is used as an argument to [`Poll.register`] and [`Poll.reregister`]
/// and is used to associate an [`Event`] with an [`Evented`] handle.
///
/// See [`Poll`] for more documentation on polling.
///
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poll.register`]: struct.Poll.html#method.register
/// [`Poll.reregister`]: struct.Poll.html#method.reregister
/// [`Event`]: ../event/struct.Event.html
/// [`Poll`]: struct.Poll.html
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub usize);

impl From<usize> for Token {
    fn from(val: usize) -> Token {
        Token(val)
    }
}

impl From<Token> for usize {
    fn from(val: Token) -> usize {
        val.0
    }
}
