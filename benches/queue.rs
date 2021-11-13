#![feature(test)]
extern crate test;

use lf_queue::Queue;

// cargo +nightly bench
#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    // cargo +nightly bench --package lf-queue --bench queue -- tests::mpmc --exact
    //
    // Latest results:
    // - MacBook Air (M1, 2020): 260,718 ns/iter (+/- 16,344)
    #[bench]
    fn mpmc(b: &mut Bencher) {
        const COUNT: usize = 1_000;
        const CONCURRENCY: usize = 4;
        let queue: Queue<usize> = Queue::new();

        b.iter(|| {
            let ths: Vec<_> = (0..CONCURRENCY)
                .map(|_| {
                    let q = queue.clone();
                    std::thread::spawn(move || {
                        for _ in 0..COUNT {
                            loop {
                                if q.pop().is_some() {
                                    break;
                                }
                            }
                        }
                    })
                })
                .map(|_| {
                    let q = queue.clone();
                    std::thread::spawn(move || {
                        for i in 0..COUNT {
                            q.push(i);
                        }
                    })
                })
                .collect();

            for th in ths {
                th.join().unwrap();
            }
        });
    }
}
