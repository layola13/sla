use std::sync::mpsc;
use std::thread;

fn main() {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        tx.send(thread::current()).unwrap();
        thread::park();
        1
    });

    let parked_thread = rx.recv().unwrap();
    parked_thread.unpark();
    println!("{}", worker.join().unwrap());
}
