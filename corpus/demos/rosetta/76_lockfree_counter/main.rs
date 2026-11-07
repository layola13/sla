use std::sync::atomic::{AtomicI32, Ordering};

fn main() {
    let counter = AtomicI32::new(1);
    counter.fetch_add(2, Ordering::SeqCst);
    println!("{}", counter.load(Ordering::SeqCst));
}
