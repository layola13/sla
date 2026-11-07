use std::sync::atomic::{AtomicI32, Ordering};

fn main() {
    let version = AtomicI32::new(0);
    let data = AtomicI32::new(0);

    version.fetch_add(1, Ordering::Release);
    data.store(10, Ordering::Release);
    version.fetch_add(1, Ordering::Release);

    let begin = version.load(Ordering::Acquire);
    let value = data.load(Ordering::Acquire);
    let end = version.load(Ordering::Acquire);

    let stable = begin == end && begin % 2 == 0;
    println!("{}", if stable { value + end } else { 0 });
}
