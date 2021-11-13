//! Prevents [false sharing](https://en.wikipedia.org/wiki/False_sharing) by adding padding
//! (unused bytes) between variables.
//!
//! As CPU memory is cached in line of some small power of two-word size (e.g., 128 aligned),
//! using padding prevents two processors to operate on independent data that might have been
//! stored on the same cache line. Such operations could result in the invalidation of the
//! whole cache line. Reducing cache invalidation on frequently accessed shared data structure
//! helps in improving the performance by reducing memory stalls and waste of system bandwidth.
//!
//! # Size and alignment
//!
//! Cache lines are assumed to be N bytes long, depending on the architecture:
//!
//! - On x86_64 and aarch64, N = 128.
//! - On all others, N = 64.
//!
//! The size of `CachePad<T>` is the smallest multiple of N bytes large enough to accommodate
//! a value of type `T`.
//!
//! # Notes
//!
//! CPU cache line:
//!
//! - MacBook Air (M1, 2020)
//!
//!   ```bash
//!   sysctl -a | grep cachelinesize
//!   hw.cachelinesize: 128
//!   ```

use std::fmt;
use std::ops::Deref;

/// Pads and aligns data to the length of a cache line.
#[cfg_attr(any(target_arch = "x86_64", target_arch = "aarch64"), repr(align(128)))]
#[cfg_attr(
    not(any(target_arch = "x86_64", target_arch = "aarch64")),
    repr(align(64))
)]
pub(crate) struct CachePad<T>(T);

impl<T> CachePad<T> {
    /// Creates a padded representation of the data aligned with the
    /// length of a cache line.
    pub(crate) fn new(t: T) -> CachePad<T> {
        CachePad(t)
    }
}

impl<T> Deref for CachePad<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Debug> fmt::Debug for CachePad<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CachePad").field(&self.0).finish()
    }
}
