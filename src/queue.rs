//! A lock-free multi-producer multi-consumer unbounded queue.

use crate::cache_pad::CachePad;
use crate::node::{Node, NODE_CAPACITY, NODE_SIZE};
use crate::slot::{DRAINING, FILLED, READING};
use crate::variant::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};
use crate::variant::sync::Arc;
use crate::variant::thread;

use std::mem::MaybeUninit;

/// A lock-free multi-producer multi-consumer unbounded queue.
#[derive(Clone, Debug)]
pub struct Queue<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Queue<T> {
    /// Creates a new [`Queue`].
    ///
    /// # Examples
    ///
    /// ```
    /// use lf_queue::Queue;
    ///
    /// let queue = Queue::<usize>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }

    /// Push an item into the [`Queue`].
    ///
    /// # Examples
    ///
    /// ```
    /// use lf_queue::Queue;
    ///
    /// let queue = Queue::<usize>::new();
    ///
    /// queue.push(1);
    /// queue.push(2);
    /// queue.push(3);
    /// ```
    pub fn push(&self, item: T) {
        self.inner.push(item)
    }

    /// Pop an item from the [`Queue`]. Returns none if the [`Queue`] is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use lf_queue::Queue;
    ///
    /// let queue = Queue::<usize>::new();
    /// for i in 0..8 {
    ///   queue.push(i);
    /// }
    ///
    /// for i in 0..8 {
    ///   assert_eq!(i, queue.pop().unwrap());
    /// }
    ///
    /// assert!(queue.pop().is_none());
    /// ```
    pub fn pop(&self) -> Option<T> {
        self.inner.pop()
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct Inner<T> {
    head: CachePad<Cursor<T>>,
    tail: CachePad<Cursor<T>>,
}

impl<T> Inner<T> {
    fn new() -> Self {
        #[cfg(not(loom))]
        let node: Node<T> = Node::UNINIT;
        #[cfg(loom)]
        let node: Node<T> = Node::new();

        let first_node: *mut CachePad<Node<T>> = Box::into_raw(Box::new(CachePad::new(node)));

        Self {
            head: CachePad::new(Cursor {
                index: AtomicUsize::new(0),
                node: AtomicPtr::new(first_node),
            }),
            tail: CachePad::new(Cursor {
                index: AtomicUsize::new(0),
                node: AtomicPtr::new(first_node),
            }),
        }
    }

    fn push(&self, item: T) {
        let mut tail_index = self.tail.index.load(Ordering::Acquire);
        let mut tail_node = self.tail.node.load(Ordering::Acquire);

        loop {
            // Defines the node container offset of the slot where the provided item should be stored.
            let offset = (tail_index >> MARK_BIT_SHIFT) % NODE_SIZE;

            // If the node container is full, we wait until the next node is
            // installed before moving forward and update our local reference.
            if offset == NODE_CAPACITY {
                thread::yield_now();
                tail_index = self.tail.index.load(Ordering::Acquire);
                tail_node = self.tail.node.load(Ordering::Acquire);
                continue;
            }

            // Increments the tail index.
            let next_tail_index = tail_index + (1 << MARK_BIT_SHIFT);
            match self.tail.index.compare_exchange_weak(
                tail_index,
                next_tail_index,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                // The tail index has been updated successfully so we can now use
                // the offset to store the item in the next available slot.
                Ok(_) => unsafe {
                    // If we're filling the last available slot of the node container,
                    // we install a new one and update both the tail and the node to point
                    // to this new node.
                    if offset + 1 == NODE_CAPACITY {
                        #[cfg(not(loom))]
                        let node: Node<T> = Node::UNINIT;
                        #[cfg(loom)]
                        let node: Node<T> = Node::new();

                        let next_node = Box::into_raw(Box::new(CachePad::new(node)));
                        self.tail.node.store(next_node, Ordering::Release);
                        let _ = self
                            .tail
                            .index
                            .fetch_add(1 << MARK_BIT_SHIFT, Ordering::Release);
                        (*tail_node).next.store(next_node, Ordering::Release);
                    }

                    // We can now safely store the provided item into the slot.
                    let slot = (*tail_node).container.get_unchecked(offset);
                    slot.item.with_mut(|p| p.write(MaybeUninit::new(item)));
                    let _ = slot.state.fetch_or(FILLED, Ordering::Release);

                    return;
                },
                // While trying to push the next item, the tail index
                // has been updated by another thread. We update our local
                // references with the value stored when we tried to make
                // the exchange and what is now the current tail's node.
                Err(current_tail_index) => {
                    tail_index = current_tail_index;
                    tail_node = self.tail.node.load(Ordering::Acquire);
                }
            }
        }
    }

    fn pop(&self) -> Option<T> {
        let mut head_index = self.head.index.load(Ordering::Acquire);
        let mut head_node = self.head.node.load(Ordering::Acquire);

        loop {
            // Defines the offset of the slot from where the next item should gathered.
            let offset = (head_index >> MARK_BIT_SHIFT) % NODE_SIZE;

            // If we reach the end of the node container, we wait until the next
            // one is installed.
            if offset == NODE_CAPACITY {
                thread::yield_now();
                head_index = self.head.index.load(Ordering::Acquire);
                head_node = self.head.node.load(Ordering::Acquire);
                continue;
            }

            // Increments the head index.
            let mut next_head_index = head_index + (1 << MARK_BIT_SHIFT);

            // If the mark bit is not set in the head index, we check if
            // there is a pending item in the queue.
            if next_head_index & MARK_BIT == 0 {
                // Sync all threads and loads the current tail cursor.
                fence(Ordering::SeqCst);
                let tail_index = self.tail.index.load(Ordering::Acquire);

                // If the head index equals the tail index, the queue is empty.
                if head_index >> MARK_BIT_SHIFT == tail_index >> MARK_BIT_SHIFT {
                    return None;
                }

                // If the head and the tail are not pointing to the same node,
                // we set the `MARK_BIT` in the head to skip cheking if there
                // is any item pending on the next iteration.
                if (head_index >> MARK_BIT_SHIFT) / NODE_SIZE
                    != (tail_index >> MARK_BIT_SHIFT) / NODE_SIZE
                {
                    next_head_index |= MARK_BIT;
                }
            }

            // Try update the head index.
            match self.head.index.compare_exchange_weak(
                head_index,
                next_head_index,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                // The head index has been updated successfully so we can now use
                // the offset to pop the next item.
                Ok(_) => unsafe {
                    // If we're returning the last item of the node container, we
                    // update the head cursor to point to the next node.
                    if offset + 1 == NODE_CAPACITY {
                        let next_node = (*head_node).wait_next();

                        // Remove the mark bit if any and increment the index.
                        let mut next_index =
                            (next_head_index & !MARK_BIT).wrapping_add(1 << MARK_BIT_SHIFT);

                        // If the next node points to another node, we can already
                        // update the index to report that the next node that will
                        // be installed is not the last one.
                        if !(*next_node).next.load(Ordering::Relaxed).is_null() {
                            next_index |= MARK_BIT;
                        }

                        self.head.node.store(next_node, Ordering::Release);
                        self.head.index.store(next_index, Ordering::Release);
                    }

                    // Reads and returns the item.
                    let slot = (*head_node).container.get_unchecked(offset);
                    slot.wait_filled();
                    let item = slot.item.with(|p| p.read().assume_init());

                    // Drain and drop the node if we've reached the end of its container, or if another
                    // thread wanted to do so but couldn't because this thread was busy reading from the slot.
                    if offset + 1 == NODE_CAPACITY {
                        Node::drain(head_node, 0);
                    } else if slot.state.fetch_or(READING, Ordering::AcqRel) & DRAINING != 0 {
                        Node::drain(head_node, offset + 1);
                    }

                    return Some(item);
                },
                // While trying to pop the next item, the head index
                // has been updated by another thread. We update our local
                // references with the value stored when we tried to make
                // the exchange and what is now the current head's node.
                Err(current_head_index) => {
                    head_index = current_head_index;
                    head_node = self.head.node.load(Ordering::Acquire);
                }
            }
        }
    }
}

#[derive(Debug)]
struct Cursor<T> {
    /// Reports the index of the next [`Slot`].
    ///
    /// Its value is used to define the offset of the slot into the current
    /// [`Node`] container by divinding it by the [`NODE_CAPACITY`].
    ///
    /// [`Slot`]: crate::slot::Slot
    index: AtomicUsize,

    /// Points to the current [`Node`].
    node: AtomicPtr<CachePad<Node<T>>>,
}

/// Defines how many lower bits are reserved for metadata.
const MARK_BIT_SHIFT: usize = 1;

/// The [`MARK_BIT`] indicates that the [`Node`] is not the last one.
///
/// The [`MARK_BIT`] helps to avoid loading the tail and head simultaneously
/// to check whether or not the queue is empty when calling the `pop` method.
///
/// [`Node`]: crate::node::Node
const MARK_BIT: usize = 1;
