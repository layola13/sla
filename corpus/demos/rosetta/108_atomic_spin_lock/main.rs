use std::sync::atomic::{AtomicI32, Ordering};

fn main() {
    let lock = AtomicI32::new(0);

    while lock
        .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {}

    lock.store(0, Ordering::Release);
    println!("1");
}
