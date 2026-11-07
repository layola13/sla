use std::thread;

fn worker_score(base: i32, delta: i32) -> i32 {
    base + delta
}

fn main() {
    let a = thread::spawn(|| worker_score(1, 2));
    let b = thread::spawn(|| worker_score(2, 3));
    let c = thread::spawn(|| worker_score(3, 4));
    let total = a.join().unwrap() + b.join().unwrap() + c.join().unwrap();
    println!("{total}");
}
