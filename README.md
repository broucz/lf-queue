# lf-queue

[![Crates.io](https://img.shields.io/crates/v/lf-queue)](https://crates.io/crates/lf-queue)
[![Documentation](https://docs.rs/lf-queue/badge.svg)](https://docs.rs/lf-queue)
[![Build Status](https://github.com/broucz/lf-queue/workflows/CI/badge.svg)](https://github.com/broucz/lf-queue/actions/workflows/ci.yml?query=branch%3Amain)
[![MIT licensed](https://img.shields.io/crates/l/lf-queue)](LICENSE)

A lock-free multi-producer multi-consumer unbounded queue.

## Examples

```toml
[dependencies]
lf-queue = "0.1"
```

Single Producer - Single Consumer:

```rust
use lf_queue::Queue;

fn main() {
    const COUNT: usize = 1_000;
    let queue: Queue<usize> = Queue::new();

    for i in 0..COUNT {
        queue.push(i);
    }

    for i in 0..COUNT {
        assert_eq!(i, queue.pop().unwrap());
    }

    assert!(queue.pop().is_none());
}
```

Multi Producer - Single Consumer:

```rust
use lf_queue::Queue;
use std::thread;

fn main() {
    const COUNT: usize = 1_000;
    const CONCURRENCY: usize = 4;

    let queue: Queue<usize> = Queue::new();

    let ths: Vec<_> = (0..CONCURRENCY)
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                for i in 0..COUNT {
                    q.push(i);
                }
            })
        })
        .collect();

    for th in ths {
        th.join().unwrap();
    }

    for _ in 0..COUNT * CONCURRENCY {
        assert!(queue.pop().is_some());
    }

    assert!(queue.pop().is_none());
}
```

Single Producer - Multi Consumer:

```rust
use lf_queue::Queue;
use std::thread;

fn main() {
    const COUNT: usize = 1_000;
    const CONCURRENCY: usize = 4;

    let queue: Queue<usize> = Queue::new();

    for i in 0..COUNT * CONCURRENCY {
        queue.push(i);
    }

    let ths: Vec<_> = (0..CONCURRENCY)
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                for _ in 0..COUNT {
                    loop {
                        if q.pop().is_some() {
                            break;
                        }
                    }
                }
            })
        })
        .collect();

    for th in ths {
        th.join().unwrap();
    }

    assert!(queue.pop().is_none());
}

```

Multi Producer - Multi Consumer:

```rust
use lf_queue::Queue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

fn main() {
    const COUNT: usize = 1_000;
    const CONCURRENCY: usize = 4;

    let queue: Queue<usize> = Queue::new();
    let items = Arc::new((0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let ths: Vec<_> = (0..CONCURRENCY)
        .map(|_| {
            let q = queue.clone();
            let its = items.clone();
            thread::spawn(move || {
                for _ in 0..COUNT {
                    let n = loop {
                        if let Some(x) = q.pop() {
                            break x;
                        } else {
                            thread::yield_now();
                        }
                    };
                    its[n].fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .map(|_| {
            let q = queue.clone();
            thread::spawn(move || {
                for i in 0..COUNT {
                    q.push(i);
                }
            })
        })
        .collect();

    for th in ths {
        th.join().unwrap();
    }

    thread::sleep(std::time::Duration::from_millis(10));

    for c in &*items {
        assert_eq!(c.load(Ordering::SeqCst), CONCURRENCY);
    }

    assert!(queue.pop().is_none());
}
```

## Acknowledgement

This implementation of a lock-free queue in Rust took inspiration from the [`concurrent-queue`](https://github.com/smol-rs/concurrent-queue) crate and aims to be used for educational purposes. The code documentation help you to discover the algorithm used to implement a concurrent lock-free queue in Rust, but might not yet be beginner-friendly. More details and learning materials will be added over time.

## License

This project is licensed under the [MIT license](LICENSE).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, shall be licensed as above, without any additional
terms or conditions.

Note that, as of now, my focus is on improving the documentation of this crate, not adding any additional feature. Please open an issue and start a discussion before working on any significant PR.

Contributions are welcome.
