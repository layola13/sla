use std::sync::mpsc;
use std::thread;

fn main() {
    let (tx, rx) = mpsc::channel();

    let left = tx.clone();
    let producer_a = thread::spawn(move || {
        left.send(1).unwrap();
        left.send(2).unwrap();
    });

    let producer_b = thread::spawn(move || {
        tx.send(3).unwrap();
        tx.send(4).unwrap();
    });

    producer_a.join().unwrap();
    producer_b.join().unwrap();

    let sum: i32 = rx.iter().take(4).sum();
    println!("{}", sum);
}
