//! Holds a collection of [`Slot`].
//!
//! The [`Node`]'s container holds [`NODE_CAPACITY`] times [`Slot`]. Each [`Slot`]
//! is used once before being scheduled for deletion. Each [`Node`] reports a pointer
//! to the next [`Node`] of the [`Queue`] if any.
//!
//! [`Queue`]: crate::queue::Queue

use crate::cache_pad::CachePad;
use crate::slot::{Slot, DRAINING, READING};
use crate::variant::sync::atomic::{AtomicPtr, Ordering};
use crate::variant::thread;

/// Holds a collection of [`Slot`].
#[derive(Debug)]
pub(crate) struct Node<T> {
    /// A pointer to the next [`Node`] of the [`Queue`] if any.
    ///
    /// [`Queue`]: crate::queue::Queue
    pub(crate) next: AtomicPtr<CachePad<Node<T>>>,

    /// A collection of [`Slot`].
    pub(crate) container: [Slot<T>; NODE_CAPACITY],
}

impl<T> Node<T> {
    /// New uninitialized [`Node`] are frequently added to the queue.
    /// Using a constant help us reducing the cost of this operation.
    #[cfg(not(loom))]
    pub(crate) const UNINIT: Node<T> = Self {
        next: AtomicPtr::new(std::ptr::null_mut()),
        container: [Slot::UNINIT; NODE_CAPACITY],
    };

    // Loom model checking can't work with constants as it needs to keep
    // track of the initialized items. In loom, `AtomicUsize::new` is therefore
    // not a `const fn`. `Slot::UNINIT` can't be use in loom context as a
    // constant item can't be initialized with calls to non constant expressions.
    #[cfg(loom)]
    pub(crate) fn new() -> Self {
        Self {
            next: AtomicPtr::new(std::ptr::null_mut()),
            container: Default::default(),
        }
    }

    /// Waits until the next pointer is set.
    pub(crate) fn wait_next(&self) -> *mut CachePad<Self> {
        loop {
            let next = self.next.load(Ordering::Acquire);
            if !next.is_null() {
                return next;
            }
            thread::yield_now();
        }
    }

    /// Drain the [`Node`] container starting from `start` and drop the [`Node`] when possible.
    pub(crate) unsafe fn drain(node: *mut CachePad<Self>, start: usize) {
        // We don't need to set the `DRAINING` bit in the last slot because that slot has
        // begun the draining of the node.
        for i in start..NODE_CAPACITY - 1 {
            let slot = unsafe { (*node).container.get_unchecked(i) };

            // Add the `DRAINING` bit if a thread is still using the slot (i.e., the
            // state is not `READING` now and after we add the `DRAINING` flag).
            // If a thread is still using the slot, it will be responsible for continuing
            // the destruction of the node.
            if slot.state.load(Ordering::Acquire) & READING == 0
                && slot.state.fetch_or(DRAINING, Ordering::AcqRel) & READING == 0
            {
                return;
            }
        }

        // No thread is using the node, it's safe to destroy it.
        drop(unsafe { Box::from_raw(node) });
    }
}

/// Each [`Node`] holds [`NODE_SIZE`] of indicies.
///
/// A [`Node`] as one reference to the next [`Node`] and [`NODE_SIZE`] - 1
/// (i.e., [`NODE_CAPACITY`]) [`Slot`] available in its container.
#[cfg(not(loom))]
pub(crate) const NODE_SIZE: usize = 8;

/// Each [`Node`] holds [`NODE_SIZE`] of indicies.
///
/// A [`Node`] as one reference to the next [`Node`] and [`NODE_SIZE`] - 1
/// (i.e., [`NODE_CAPACITY`]) [`Slot`] available in its container.
///
/// When using loom, we shrink the size of the local queue. This shouldn't impact
/// logic, but allows loom to test more edge cases in a reasonable a mount of time.
#[cfg(loom)]
pub(crate) const NODE_SIZE: usize = 4;

/// Reports the capacity (max number of item), a [`Node`] container can hold.
pub(crate) const NODE_CAPACITY: usize = NODE_SIZE - 1;
