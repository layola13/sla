use std::sync::{Arc, Barrier};
use std::thread;

fn main() {
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                1
            })
        })
        .collect();

    let arrived: i32 = handles.into_iter().map(|handle| handle.join().unwrap()).sum();
    println!("{}", arrived);
}
