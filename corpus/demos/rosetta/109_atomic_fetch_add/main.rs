use std::sync::atomic::{AtomicI32, Ordering};

fn main() {
    let value = AtomicI32::new(5);
    let old = value.fetch_add(3, Ordering::SeqCst);
    let new = value.load(Ordering::SeqCst);
    println!("{}", old + new);
}
