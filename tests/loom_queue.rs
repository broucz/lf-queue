#![cfg(loom)]

use lf_queue::Queue;
use loom::thread;

// When using the `--cfg loom` flag, the node container is equal to 4. Below test uses an item count equal to 5
// to test both node addition and removal.
//
// Run all tests:
//
// RUSTFLAGS="--cfg loom" cargo test --package lf-queue --test loom_queue --release
//
// Note that running some of these tests may a few seconds. Add `LOOM_MAX_PREEMPTIONS=2` (or =3) to the command
// above to reduce the test complexity and so its duration.

// RUSTFLAGS="--cfg loom" cargo test --package lf-queue --test loom_queue --release -- test_mpsc --exact
#[test]
fn test_mpsc() {
    loom::model(|| {
        const COUNT: usize = 5;
        let queue: Queue<usize> = Queue::new();

        let q1 = queue.clone();
        let th1 = thread::spawn(move || {
            for i in 0..3 {
                q1.push(i);
            }
        });

        let q2 = queue.clone();
        let th2 = thread::spawn(move || {
            for i in 3..5 {
                q2.push(i);
            }
        });

        th1.join().unwrap();
        th2.join().unwrap();

        for _ in 0..COUNT {
            assert!(queue.pop().is_some());
        }
    });
}

// RUSTFLAGS="--cfg loom" cargo test --package lf-queue --test loom_queue --release -- test_spmc --exact
#[test]
fn test_spmc() {
    loom::model(|| {
        const COUNT: usize = 5;
        let queue: Queue<usize> = Queue::new();

        for i in 0..COUNT {
            queue.push(i);
        }

        let mut n = 0;

        let q1 = queue.clone();
        let th1 = thread::spawn(move || {
            let mut x = 0;
            while q1.pop().is_some() {
                x += 1;
            }

            x
        });

        let q2 = queue.clone();
        let th2 = thread::spawn(move || {
            let mut x = 0;
            while q2.pop().is_some() {
                x += 1;
            }

            x
        });

        n += th1.join().unwrap();
        n += th2.join().unwrap();

        assert_eq!(n, COUNT);
    });
}

// RUSTFLAGS="--cfg loom" cargo test --package lf-queue --test loom_queue --release -- test_concurrent_push_and_pop --exact
#[test]
fn test_concurrent_push_and_pop() {
    loom::model(|| {
        const COUNT: usize = 5;
        let queue: Queue<usize> = Queue::new();

        let q1 = queue.clone();
        let th1 = thread::spawn(move || {
            for i in 0..COUNT {
                q1.push(i);
            }
        });

        let q2 = queue.clone();
        let th2 = thread::spawn(move || {
            for _ in 0..COUNT {
                loop {
                    if q2.pop().is_some() {
                        break;
                    } else {
                        // Loom scheduler is, by design, not fair. Yielding here indicates to Loom
                        // that this thread needs another one to be scheduled before making progress.
                        // In our case, some loom executions will be blocked when we reach the end of
                        // a node container which requires the next node to be installed before making
                        // progress.
                        thread::yield_now()
                    }
                }
            }
        });

        th1.join().unwrap();
        th2.join().unwrap();
    });
}
