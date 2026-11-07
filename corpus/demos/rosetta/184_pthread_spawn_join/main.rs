use std::thread;

fn worker(value: i32) -> i32 {
    value + 4
}

fn main() {
    let handle = thread::spawn(|| worker(1));
    let joined = handle.join().unwrap();
    println!("{}", joined);
}
