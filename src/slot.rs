//! Holds an item of the [`Queue`].
//!
//! The [`Slot`]'s state uses below bit flags to report its progress:
//!
//! ```txt
//! INITIAL  0b00000000
//! FILLED   0b00000001 -> Bit flag added after as successful write of the item into the slot.
//! READING  0b00000010 -> Bit flag added from the time a thread starts to read the value.
//! DRAINING 0b00000100 -> Added when a draining of the slot has been scheduled.
//! ```
//!
//! The state of the [`Slot`] can only move forward, resulting in bit flags being added in below
//! order:
//!
//! ```txt
//! INITIAL             0b00000000
//! INITIAL -> FILLED   0b00000001
//! FILLED  -> READING  0b00000011
//! READING -> DRAINING 0b00000111
//! ```
//!
//! [`Node`]: crate::node::Node
//! [`NODE_CAPACITY`]: crate::node::NODE_CAPACITY
//! [`Queue`]: crate::queue::Queue

use crate::variant::cell::UnsafeCell;
use crate::variant::sync::atomic::AtomicUsize;
use crate::variant::thread;

use std::mem::MaybeUninit;
use std::sync::atomic::Ordering;

/// Holds an item of the [`Queue`].
///
/// [`Queue`]: crate::queue::Queue
#[derive(Debug)]
pub(crate) struct Slot<T> {
    /// Holds an item pushed to the [`Queue`].
    ///
    /// [`Queue`]: crate::queue::Queue
    pub(crate) item: UnsafeCell<MaybeUninit<T>>,

    /// Reports the state of the [`Slot`].
    pub(crate) state: AtomicUsize,
}

impl<T> Slot<T> {
    /// When creating a new [`Node`], [`NODE_CAPACITY`] unititialized
    /// [`Slot`] are added to the [`Node`] container. Using a constant
    /// help us reducing the cost of this operation.
    ///
    /// [`Node`]: crate::node::Node
    /// [`NODE_CAPACITY`]: crate::node::NODE_CAPACITY
    #[cfg(not(loom))]
    pub(crate) const UNINIT: Slot<T> = Self {
        item: UnsafeCell::new(MaybeUninit::uninit()),
        state: AtomicUsize::new(0),
    };

    // Loom model checking can't work with constants as it needs to keep
    // track of the initialized items. In loom, `AtomicUsize::new` is therefore
    // not a `const fn`. `Slot::UNINIT` can't be use in loom context as a
    // constant item can't be initialized with calls to non constant expressions.
    #[cfg(loom)]
    pub(crate) fn new() -> Self {
        Self {
            item: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicUsize::new(0),
        }
    }

    /// Waits until the state has a [`FILLED`] state.
    pub(crate) fn wait_filled(&self) {
        while self.state.load(Ordering::Acquire) & FILLED == 0 {
            thread::yield_now()
        }
    }
}

// As we can't use compile time constants and function to create a new
// Node container in loom, we implement the Default trait to simplify
// the creation of a new Node container with uninitialized Slots.
#[cfg(loom)]
impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Bit flag added when the [`Slot`] holds an item.
pub(crate) const FILLED: usize = 1;

/// Bit flag added when the [`Slot`] item is being used by a thread.
pub(crate) const READING: usize = 2;

/// Bit flag added when the [`Slot`] is scheduled for deletion.
pub(crate) const DRAINING: usize = 4;
