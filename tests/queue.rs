use lf_queue::Queue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

// cargo test --package lf-queue --test queue -- test_spsc --exact --nocapture
#[test]
fn test_spsc() {
    const COUNT: usize = 7 * 3;
    let queue: Queue<usize> = Queue::new();

    for i in 0..COUNT {
        queue.push(i);
    }

    for i in 0..COUNT {
        assert_eq!(i, queue.pop().unwrap());
    }

    assert!(queue.pop().is_none());
}

// cargo test --package lf-queue --test queue -- test_mpsc --exact --nocapture
#[test]
fn test_mpsc() {
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

// cargo test --package lf-queue --test queue -- test_spmc --exact --nocapture
#[test]
fn test_spmc() {
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

// cargo test --package lf-queue --test queue -- test_mpmc --exact --nocapture
#[test]
fn test_mpmc() {
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
