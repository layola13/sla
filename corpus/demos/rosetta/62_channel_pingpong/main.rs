use std::sync::mpsc;

fn round_trip() -> i32 {
    let (tx, rx) = mpsc::channel();
    tx.send(8).unwrap();
    let value = rx.recv().unwrap();
    tx.send(value + 1).unwrap();
    rx.recv().unwrap()
}

fn main() {
    println!("{}", round_trip());
}
