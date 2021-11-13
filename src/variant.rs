//! Switch from [`std`] to [`loom`] for [`std::cell`], [`std::sync`] and [`std::thread`] when using the `--cfg loom` flag.
//!
//! [`loom`]: https://docs.rs/loom/

#[cfg(not(loom))]
pub(crate) mod cell {
    #[derive(Debug)]
    #[repr(transparent)]
    pub(crate) struct UnsafeCell<T>(std::cell::UnsafeCell<T>);

    impl<T> UnsafeCell<T> {
        pub(crate) const fn new(data: T) -> UnsafeCell<T> {
            UnsafeCell(std::cell::UnsafeCell::new(data))
        }

        pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
            f(self.0.get())
        }

        pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
            f(self.0.get())
        }
    }
}

#[cfg(not(loom))]
pub(crate) mod sync {
    pub(crate) use std::sync::Arc;

    pub(crate) mod atomic {
        pub(crate) use std::sync::atomic::{fence, AtomicPtr, Ordering};

        #[derive(Debug)]
        #[repr(transparent)]
        pub(crate) struct AtomicUsize(std::sync::atomic::AtomicUsize);

        impl AtomicUsize {
            pub(crate) const fn new(v: usize) -> Self {
                Self(std::sync::atomic::AtomicUsize::new(v))
            }

            pub(crate) fn load(&self, order: Ordering) -> usize {
                self.0.load(order)
            }

            pub(crate) fn store(&self, val: usize, order: Ordering) {
                self.0.store(val, order)
            }

            pub(crate) fn compare_exchange_weak(
                &self,
                current: usize,
                new: usize,
                success: Ordering,
                failure: Ordering,
            ) -> Result<usize, usize> {
                self.0.compare_exchange_weak(current, new, success, failure)
            }

            pub(crate) fn fetch_add(&self, val: usize, order: Ordering) -> usize {
                self.0.fetch_add(val, order)
            }

            pub(crate) fn fetch_or(&self, val: usize, order: Ordering) -> usize {
                self.0.fetch_or(val, order)
            }
        }
    }
}

#[cfg(not(loom))]
pub(crate) use std::thread;

#[cfg(loom)]
pub(crate) use loom::cell;
#[cfg(loom)]
pub(crate) use loom::sync;
#[cfg(loom)]
pub(crate) use loom::thread;
