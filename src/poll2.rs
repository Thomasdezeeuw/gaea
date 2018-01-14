use std::{io, ptr, mem};
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicPtr, AtomicBool, Ordering};

use super::{sys, Token};
use super::event_imp::{Ready, PollOpt};
use event::Event;
use poll::Poll;

use registration::*;

/// Used to associate an IO type with a Selector
#[derive(Debug)]
pub struct SelectorId {
    id: AtomicUsize,
}

#[derive(Clone)]
pub(crate) struct ReadinessQueue {
    pub(crate) inner: Arc<ReadinessQueueInner>,
}

unsafe impl Send for ReadinessQueue {}
unsafe impl Sync for ReadinessQueue {}

pub(crate) struct ReadinessQueueInner {
    // Used to wake up `Poll` when readiness is set in another thread.
    pub(crate) awakener: sys::Awakener,

    // Head of the MPSC queue used to signal readiness to `Poll::poll`.
    head_readiness: AtomicPtr<ReadinessNode>,

    // Tail of the readiness queue.
    //
    // Only accessed by Poll::poll. Coordination will be handled by the poll fn
    tail_readiness: UnsafeCell<*mut ReadinessNode>,

    // Fake readiness node used to punctuate the end of the readiness queue.
    // Before attempting to read from the queue, this node is inserted in order
    // to partition the queue between nodes that are "owned" by the dequeue end
    // and nodes that will be pushed on by producers.
    end_marker: Box<ReadinessNode>,

    // Similar to `end_marker`, but this node signals to producers that `Poll`
    // has gone to sleep and must be woken up.
    sleep_marker: Box<ReadinessNode>,

    // Similar to `end_marker`, but the node signals that the queue is closed.
    // This happens when `ReadyQueue` is dropped and signals to producers that
    // the nodes should no longer be pushed into the queue.
    closed_marker: Box<ReadinessNode>,
}

/// Node shared by a `Registration` / `SetReadiness` pair as well as the node
/// queued into the MPSC channel.
#[derive(Debug)]
pub(crate) struct ReadinessNode {
    // Node state, see struct docs for `ReadinessState`
    //
    // This variable is the primary point of coordination between all the
    // various threads concurrently accessing the node.
    pub(crate) state: AtomicState,

    // The registration token cannot fit into the `state` variable, so it is
    // broken out here. In order to atomically update both the state and token
    // we have to jump through a few hoops.
    //
    // First, `state` includes `token_read_pos` and `token_write_pos`. These can
    // either be 0, 1, or 2 which represent a token slot. `token_write_pos` is
    // the token slot that contains the most up to date registration token.
    // `token_read_pos` is the token slot that `poll` is currently reading from.
    //
    // When a call to `update` includes a different token than the one currently
    // associated with the registration (token_write_pos), first an unused token
    // slot is found. The unused slot is the one not represented by
    // `token_read_pos` OR `token_write_pos`. The new token is written to this
    // slot, then `state` is updated with the new `token_write_pos` value. This
    // requires that there is only a *single* concurrent call to `update`.
    //
    // When `poll` reads a node state, it checks that `token_read_pos` matches
    // `token_write_pos`. If they do not match, then it atomically updates
    // `state` such that `token_read_pos` is set to `token_write_pos`. It will
    // then read the token at the newly updated `token_read_pos`.
    pub(crate) token_0: UnsafeCell<Token>,
    pub(crate) token_1: UnsafeCell<Token>,
    pub(crate) token_2: UnsafeCell<Token>,

    // Used when the node is queued in the readiness linked list. Accessing
    // this field requires winning the "queue" lock
    pub(crate) next_readiness: AtomicPtr<ReadinessNode>,

    // Ensures that there is only one concurrent call to `update`.
    //
    // Each call to `update` will attempt to swap `update_lock` from `false` to
    // `true`. If the CAS succeeds, the thread has obtained the update lock. If
    // the CAS fails, then the `update` call returns immediately and the update
    // is discarded.
    pub(crate) update_lock: AtomicBool,

    // Pointer to Arc<ReadinessQueueInner>
    pub(crate) readiness_queue: AtomicPtr<()>,

    // Tracks the number of `ReadyRef` pointers
    pub(crate) ref_count: AtomicUsize,
}

/// Stores the ReadinessNode state in an AtomicUsize. This wrapper around the
/// atomic variable handles encoding / decoding `ReadinessState` values.
#[derive(Debug)]
pub(crate) struct AtomicState {
    inner: AtomicUsize,
}

const MASK_2: usize = 4 - 1;
const MASK_4: usize = 16 - 1;
const MASK_8: usize = 256 - 1;
const QUEUED_MASK: usize = 1 << QUEUED_SHIFT;
const DROPPED_MASK: usize = 1 << DROPPED_SHIFT;

const READINESS_SHIFT: usize = 0;
const INTEREST_SHIFT: usize = 8;
const POLL_OPT_SHIFT: usize = 16;
const TOKEN_RD_SHIFT: usize = 20;
const TOKEN_WR_SHIFT: usize = 22;
const QUEUED_SHIFT: usize = 24;
const DROPPED_SHIFT: usize = 25;

/// Tracks all state for a single `ReadinessNode`. The state is packed into a
/// `usize` variable from low to high bit as follows:
///
/// 8 bits: Registration current readiness
/// 8 bits: Registration interest
/// 4 bits: Poll options
/// 2 bits: Token position currently being read from by `poll`
/// 2 bits: Token position last written to by `update`
/// 1 bit:  Queued flag, set when node is being pushed into MPSC queue.
/// 1 bit:  Dropped flag, set when all `Registration` handles have been dropped.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ReadinessState(usize);

/// Returned by `dequeue_node`. Represents the different states as described by
/// the queue documentation on 1024cores.net.
#[derive(Debug)]
enum Dequeue {
    Data(*mut ReadinessNode),
    Empty,
    Inconsistent,
}

pub(crate) const MAX_REFCOUNT: usize = (::std::isize::MAX) as usize;

// ===== Accessors for internal usage =====

pub fn selector(poll: &Poll) -> &sys::Selector {
    &poll.selector
}

/*
 *
 * ===== Registration =====
 *
 */

// TODO: get rid of this, windows depends on it for now
#[allow(dead_code)]
pub fn new_registration(poll: &Poll, token: Token, ready: Ready, opt: PollOpt)
        -> (Registration, SetReadiness)
{
    Registration::new_priv(poll, token, ready, opt)
}

/*
 *
 * ===== ReadinessQueue =====
 *
 */

impl ReadinessQueue {
    /// Create a new `ReadinessQueue`.
    pub(crate) fn new() -> io::Result<ReadinessQueue> {
        is_send::<Self>();
        is_sync::<Self>();

        let end_marker = Box::new(ReadinessNode::marker());
        let sleep_marker = Box::new(ReadinessNode::marker());
        let closed_marker = Box::new(ReadinessNode::marker());

        let ptr = &*end_marker as *const _ as *mut _;

        Ok(ReadinessQueue {
            inner: Arc::new(ReadinessQueueInner {
                awakener: sys::Awakener::new()?,
                head_readiness: AtomicPtr::new(ptr),
                tail_readiness: UnsafeCell::new(ptr),
                end_marker: end_marker,
                sleep_marker: sleep_marker,
                closed_marker: closed_marker,
            })
        })
    }

    /// Poll the queue for new events
    pub(crate) fn poll(&self, dst: &mut sys::Events) {
        // `until` is set with the first node that gets re-enqueued due to being
        // set to have level-triggered notifications. This prevents an infinite
        // loop where `Poll::poll` will keep dequeuing nodes it enqueues.
        let mut until = ptr::null_mut();

        'outer:
        while dst.len() < dst.capacity() {
            // Dequeue a node. If the queue is in an inconsistent state, then
            // stop polling. `Poll::poll` will be called again shortly and enter
            // a syscall, which should be enough to enable the other thread to
            // finish the queuing process.
            let ptr = match unsafe { self.inner.dequeue_node(until) } {
                Dequeue::Empty | Dequeue::Inconsistent => break,
                Dequeue::Data(ptr) => ptr,
            };

            let node = unsafe { &*ptr };

            // Read the node state with Acquire ordering. This allows reading
            // the token variables.
            let mut state = node.state.load(Ordering::Acquire);
            let mut next;
            let mut readiness;
            let mut opt;

            loop {
                // Build up any changes to the readiness node's state and
                // attempt the CAS at the end
                next = state;

                // Given that the node was just read from the queue, the
                // `queued` flag should still be set.
                debug_assert!(state.is_queued());

                // The dropped flag means we need to release the node and
                // perform no further processing on it.
                if state.is_dropped() {
                    // Release the node and continue
                    release_node(ptr);
                    continue 'outer;
                }

                // Process the node
                readiness = state.effective_readiness();
                opt = state.poll_opt();

                if opt.is_edge() {
                    // Mark the node as dequeued
                    next.set_dequeued();

                    if opt.is_oneshot() && !readiness.is_empty() {
                        next.disarm();
                    }
                } else if readiness.is_empty() {
                    next.set_dequeued();
                }

                // Ensure `token_read_pos` is set to `token_write_pos` so that
                // we read the most up to date token value.
                next.update_token_read_pos();

                if state == next {
                    break;
                }

                let actual = node.state.compare_and_swap(state, next, Ordering::AcqRel);

                if actual == state {
                    break;
                }

                state = actual;
            }

            // If the queued flag is still set, then the node must be requeued.
            // This typically happens when using level-triggered notifications.
            if next.is_queued() {
                if until.is_null() {
                    // We never want to see the node again
                    until = ptr;
                }

                // Requeue the node
                self.inner.enqueue_node(node);
            }

            if !readiness.is_empty() {
                // Get the token
                let token = unsafe { token(node, next.token_read_pos()) };

                // Push the event
                dst.push_event(Event::new(readiness, token));
            }
        }
    }

    /// Prepare the queue for the `Poll::poll` thread to block in the system
    /// selector. This involves changing `head_readiness` to `sleep_marker`.
    /// Returns true if successful and `poll` can block.
    pub(crate) fn prepare_for_sleep(&self) -> bool {
        let end_marker = self.inner.end_marker();
        let sleep_marker = self.inner.sleep_marker();

        let tail = unsafe { *self.inner.tail_readiness.get() };

        // If the tail is currently set to the sleep_marker, then check if the
        // head is as well. If it is, then the queue is currently ready to
        // sleep. If it is not, then the queue is not empty and there should be
        // no sleeping.
        if tail == sleep_marker {
            return self.inner.head_readiness.load(Ordering::Acquire) == sleep_marker;
        }

        // If the tail is not currently set to `end_marker`, then the queue is
        // not empty.
        if tail != end_marker {
            return false;
        }

        // The sleep marker is *not* currently in the readiness queue.
        //
        // The sleep marker is only inserted in this function. It is also only
        // inserted in the tail position. This is guaranteed by first checking
        // that the end marker is in the tail position, pushing the sleep marker
        // after the end marker, then removing the end marker.
        //
        // Before inserting a node into the queue, the next pointer has to be
        // set to null. Again, this is only safe to do when the node is not
        // currently in the queue, but we already have ensured this.
        self.inner.sleep_marker.next_readiness.store(ptr::null_mut(), Ordering::Relaxed);

        let actual = self.inner.head_readiness.compare_and_swap(
            end_marker, sleep_marker, Ordering::AcqRel);

        debug_assert!(actual != sleep_marker);

        if actual != end_marker {
            // The readiness queue is not empty
            return false;
        }

        // The current tail should be pointing to `end_marker`
        debug_assert!(unsafe { *self.inner.tail_readiness.get() == end_marker });
        // The `end_marker` next pointer should be null
        debug_assert!(self.inner.end_marker.next_readiness.load(Ordering::Relaxed).is_null());

        // Update tail pointer.
        unsafe { *self.inner.tail_readiness.get() = sleep_marker; }
        true
    }
}

impl Drop for ReadinessQueue {
    fn drop(&mut self) {
        // Close the queue by enqueuing the closed node
        self.inner.enqueue_node(&*self.inner.closed_marker);

        loop {
            // Free any nodes that happen to be left in the readiness queue
            let ptr = match unsafe { self.inner.dequeue_node(ptr::null_mut()) } {
                Dequeue::Empty => break,
                Dequeue::Inconsistent => {
                    // This really shouldn't be possible as all other handles to
                    // `ReadinessQueueInner` are dropped, but handle this by
                    // spinning I guess?
                    continue;
                }
                Dequeue::Data(ptr) => ptr,
            };

            let node = unsafe { &*ptr };

            let state = node.state.load(Ordering::Acquire);

            debug_assert!(state.is_queued());

            release_node(ptr);
        }
    }
}

impl ReadinessQueueInner {
    fn wakeup(&self) -> io::Result<()> {
        self.awakener.wakeup()
    }

    /// Prepend the given node to the head of the readiness queue. This is done
    /// with relaxed ordering. Returns true if `Poll` needs to be woken up.
    fn enqueue_node_with_wakeup(&self, node: &ReadinessNode) -> io::Result<()> {
        if self.enqueue_node(node) {
            self.wakeup()?;
        }

        Ok(())
    }

    /// Push the node into the readiness queue
    fn enqueue_node(&self, node: &ReadinessNode) -> bool {
        // This is the 1024cores.net intrusive MPSC queue [1] "push" function.
        let node_ptr = node as *const _ as *mut _;

        // Relaxed used as the ordering is "released" when swapping
        // `head_readiness`
        node.next_readiness.store(ptr::null_mut(), Ordering::Relaxed);

        unsafe {
            let mut prev = self.head_readiness.load(Ordering::Acquire);

            loop {
                if prev == self.closed_marker() {
                    debug_assert!(node_ptr != self.closed_marker());
                    // debug_assert!(node_ptr != self.end_marker());
                    debug_assert!(node_ptr != self.sleep_marker());

                    if node_ptr != self.end_marker() {
                        // The readiness queue is shutdown, but the enqueue flag was
                        // set. This means that we are responsible for decrementing
                        // the ready queue's ref count
                        debug_assert!(node.ref_count.load(Ordering::Relaxed) >= 2);
                        release_node(node_ptr);
                    }

                    return false;
                }

                let act = self.head_readiness.compare_and_swap(prev, node_ptr, Ordering::AcqRel);

                if prev == act {
                    break;
                }

                prev = act;
            }

            debug_assert!((*prev).next_readiness.load(Ordering::Relaxed).is_null());

            (*prev).next_readiness.store(node_ptr, Ordering::Release);

            prev == self.sleep_marker()
        }
    }

    /// Must only be called in `poll` or `drop`
    unsafe fn dequeue_node(&self, until: *mut ReadinessNode) -> Dequeue {
        // This is the 1024cores.net intrusive MPSC queue [1] "pop" function
        // with the modifications mentioned at the top of the file.
        let mut tail = *self.tail_readiness.get();
        let mut next = (*tail).next_readiness.load(Ordering::Acquire);

        if tail == self.end_marker() || tail == self.sleep_marker() || tail == self.closed_marker() {
            if next.is_null() {
                return Dequeue::Empty;
            }

            *self.tail_readiness.get() = next;
            tail = next;
            next = (*next).next_readiness.load(Ordering::Acquire);
        }

        // Only need to check `until` at this point. `until` is either null,
        // which will never match tail OR it is a node that was pushed by
        // the current thread. This means that either:
        //
        // 1) The queue is inconsistent, which is handled explicitly
        // 2) We encounter `until` at this point in dequeue
        // 3) we will pop a different node
        if tail == until {
            return Dequeue::Empty;
        }

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            return Dequeue::Data(tail);
        }

        if self.head_readiness.load(Ordering::Acquire) != tail {
            return Dequeue::Inconsistent;
        }

        // Push the stub node
        self.enqueue_node(&*self.end_marker);

        next = (*tail).next_readiness.load(Ordering::Acquire);

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            return Dequeue::Data(tail);
        }

        Dequeue::Inconsistent
    }

    fn end_marker(&self) -> *mut ReadinessNode {
        &*self.end_marker as *const ReadinessNode as *mut ReadinessNode
    }

    fn sleep_marker(&self) -> *mut ReadinessNode {
        &*self.sleep_marker as *const ReadinessNode as *mut ReadinessNode
    }

    fn closed_marker(&self) -> *mut ReadinessNode {
        &*self.closed_marker as *const ReadinessNode as *mut ReadinessNode
    }
}

impl ReadinessNode {
    /// Return a new `ReadinessNode`, initialized with a ref_count of 3.
    pub(crate) fn new(queue: *mut (),
           token: Token,
           interest: Ready,
           opt: PollOpt,
           ref_count: usize) -> ReadinessNode
    {
        ReadinessNode {
            state: AtomicState::new(interest, opt),
            // Only the first token is set, the others are initialized to 0
            token_0: UnsafeCell::new(token),
            token_1: UnsafeCell::new(Token(0)),
            token_2: UnsafeCell::new(Token(0)),
            next_readiness: AtomicPtr::new(ptr::null_mut()),
            update_lock: AtomicBool::new(false),
            readiness_queue: AtomicPtr::new(queue),
            ref_count: AtomicUsize::new(ref_count),
        }
    }

    fn marker() -> ReadinessNode {
        ReadinessNode {
            state: AtomicState::new(Ready::empty(), PollOpt::empty()),
            token_0: UnsafeCell::new(Token(0)),
            token_1: UnsafeCell::new(Token(0)),
            token_2: UnsafeCell::new(Token(0)),
            next_readiness: AtomicPtr::new(ptr::null_mut()),
            update_lock: AtomicBool::new(false),
            readiness_queue: AtomicPtr::new(ptr::null_mut()),
            ref_count: AtomicUsize::new(0),
        }
    }

    pub(crate) fn enqueue_with_wakeup(&self) -> io::Result<()> {
        let queue = self.readiness_queue.load(Ordering::Acquire);

        if queue.is_null() {
            // Not associated with a queue, nothing to do
            return Ok(());
        }

        enqueue_with_wakeup(queue, self)
    }
}

pub(crate) fn enqueue_with_wakeup(queue: *mut (), node: &ReadinessNode) -> io::Result<()> {
    debug_assert!(!queue.is_null());
    // This is ugly... but we don't want to bump the ref count.
    let queue: &Arc<ReadinessQueueInner> = unsafe { mem::transmute(&queue) };
    queue.enqueue_node_with_wakeup(node)
}

pub(crate) unsafe fn token(node: &ReadinessNode, pos: usize) -> Token {
    match pos {
        0 => *node.token_0.get(),
        1 => *node.token_1.get(),
        2 => *node.token_2.get(),
        _ => unreachable!(),
    }
}

pub(crate) fn release_node(ptr: *mut ReadinessNode) {
    unsafe {
        // `AcqRel` synchronizes with other `release_node` functions and ensures
        // that the drop happens after any reads / writes on other threads.
        if (*ptr).ref_count.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }

        let node = Box::from_raw(ptr);

        // Decrement the readiness_queue Arc
        let queue = node.readiness_queue.load(Ordering::Acquire);

        if queue.is_null() {
            return;
        }

        let _: Arc<ReadinessQueueInner> = mem::transmute(queue);
    }
}

impl AtomicState {
    fn new(interest: Ready, opt: PollOpt) -> AtomicState {
        let state = ReadinessState::new(interest, opt);

        AtomicState {
            inner: AtomicUsize::new(state.into()),
        }
    }

    /// Loads the current `ReadinessState`
    pub(crate) fn load(&self, order: Ordering) -> ReadinessState {
        self.inner.load(order).into()
    }

    /// Stores a state if the current state is the same as `current`.
    pub(crate) fn compare_and_swap(&self, current: ReadinessState, new: ReadinessState, order: Ordering) -> ReadinessState {
        self.inner.compare_and_swap(current.into(), new.into(), order).into()
    }

    // Returns `true` if the node should be queued
    pub(crate) fn flag_as_dropped(&self) -> bool {
        let prev: ReadinessState = self.inner.fetch_or(DROPPED_MASK | QUEUED_MASK, Ordering::Release).into();
        // The flag should not have been previously set
        debug_assert!(!prev.is_dropped());

        !prev.is_queued()
    }
}

impl ReadinessState {
    // Create a `ReadinessState` initialized with the provided arguments
    #[inline]
    fn new(interest: Ready, opt: PollOpt) -> ReadinessState {
        let interest = interest.bits() as usize;
        let opt = opt.bits() as usize;

        debug_assert!(interest <= MASK_8);
        debug_assert!(opt <= MASK_4);

        let mut val = interest << INTEREST_SHIFT;
        val |= opt << POLL_OPT_SHIFT;

        ReadinessState(val)
    }

    #[inline]
    fn get(&self, mask: usize, shift: usize) -> usize{
        (self.0 >> shift) & mask
    }

    #[inline]
    fn set(&mut self, val: usize, mask: usize, shift: usize) {
        self.0 = (self.0 & !(mask << shift)) | (val << shift)
    }

    /// Get the readiness.
    #[inline]
    pub(crate) fn readiness(&self) -> Ready {
        let v = self.get(MASK_8, READINESS_SHIFT);
        Ready::from_bits_truncate(v as u8)
    }

    #[inline]
    pub(crate) fn effective_readiness(&self) -> Ready {
        self.readiness() & self.interest()
    }

    /// Set the readiness.
    #[inline]
    pub(crate) fn set_readiness(&mut self, v: Ready) {
        self.set(v.bits() as usize, MASK_8, READINESS_SHIFT);
    }

    /// Get the interest.
    #[inline]
    pub(crate) fn interest(&self) -> Ready {
        let v = self.get(MASK_8, INTEREST_SHIFT);
        Ready::from_bits_truncate(v as u8)
    }

    /// Set the interest
    #[inline]
    pub(crate) fn set_interest(&mut self, v: Ready) {
        self.set(v.bits() as usize, MASK_8, INTEREST_SHIFT);
    }

    #[inline]
    pub(crate) fn disarm(&mut self) {
        self.set_interest(Ready::empty());
    }

    /// Get the poll options
    #[inline]
    pub(crate) fn poll_opt(&self) -> PollOpt {
        let v = self.get(MASK_4, POLL_OPT_SHIFT);
        PollOpt::from_bits_truncate(v as u8)
    }

    /// Set the poll options
    #[inline]
    pub(crate) fn set_poll_opt(&mut self, v: PollOpt) {
        self.set(v.bits() as usize, MASK_4, POLL_OPT_SHIFT);
    }

    #[inline]
    pub(crate) fn is_queued(&self) -> bool {
        self.0 & QUEUED_MASK == QUEUED_MASK
    }

    /// Set the queued flag
    #[inline]
    pub(crate) fn set_queued(&mut self) {
        // Dropped nodes should never be queued
        debug_assert!(!self.is_dropped());
        self.0 |= QUEUED_MASK;
    }

    #[inline]
    pub(crate) fn set_dequeued(&mut self) {
        debug_assert!(self.is_queued());
        self.0 &= !QUEUED_MASK
    }

    #[inline]
    pub(crate) fn is_dropped(&self) -> bool {
        self.0 & DROPPED_MASK == DROPPED_MASK
    }

    #[inline]
    fn token_read_pos(&self) -> usize {
        self.get(MASK_2, TOKEN_RD_SHIFT)
    }

    #[inline]
    pub(crate) fn token_write_pos(&self) -> usize {
        self.get(MASK_2, TOKEN_WR_SHIFT)
    }

    #[inline]
    pub(crate) fn next_token_pos(&self) -> usize {
        let rd = self.token_read_pos();
        let wr = self.token_write_pos();

        match wr {
            0 => {
                match rd {
                    1 => 2,
                    2 => 1,
                    0 => 1,
                    _ => unreachable!(),
                }
            }
            1 => {
                match rd {
                    0 => 2,
                    2 => 0,
                    1 => 2,
                    _ => unreachable!(),
                }
            }
            2 => {
                match rd {
                    0 => 1,
                    1 => 0,
                    2 => 0,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    #[inline]
    pub(crate) fn set_token_write_pos(&mut self, val: usize) {
        self.set(val, MASK_2, TOKEN_WR_SHIFT);
    }

    #[inline]
    pub(crate) fn update_token_read_pos(&mut self) {
        let val = self.token_write_pos();
        self.set(val, MASK_2, TOKEN_RD_SHIFT);
    }
}

impl From<ReadinessState> for usize {
    fn from(src: ReadinessState) -> usize {
        src.0
    }
}

impl From<usize> for ReadinessState {
    fn from(src: usize) -> ReadinessState {
        ReadinessState(src)
    }
}

fn is_send<T: Send>() {}
fn is_sync<T: Sync>() {}

impl SelectorId {
    pub fn new() -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(0),
        }
    }

    pub fn associate_selector(&self, poll: &Poll) -> io::Result<()> {
        let selector_id = self.id.load(Ordering::SeqCst);

        if selector_id != 0 && selector_id != poll.selector.id() {
            Err(io::Error::new(io::ErrorKind::Other, "socket already registered"))
        } else {
            self.id.store(poll.selector.id(), Ordering::SeqCst);
            Ok(())
        }
    }
}

impl Clone for SelectorId {
    fn clone(&self) -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(self.id.load(Ordering::SeqCst)),
        }
    }
}
