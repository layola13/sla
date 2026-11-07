use std::sync::{Arc, Condvar, Mutex};
use std::thread;

fn main() {
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let notifier = Arc::clone(&pair);

    thread::spawn(move || {
        let (lock, condvar) = &*notifier;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        condvar.notify_one();
    });

    let (lock, condvar) = &*pair;
    let ready = condvar
        .wait_while(lock.lock().unwrap(), |ready| !*ready)
        .unwrap();
    let value = if *ready { 4 } else { 0 };
    println!("{}", value);
}
