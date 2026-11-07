use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
    let shared = Arc::new(RwLock::new(1i32));

    let first_reader = {
        let shared = Arc::clone(&shared);
        thread::spawn(move || *shared.read().unwrap())
    };

    let first = first_reader.join().unwrap();
    {
        let mut writer = shared.write().unwrap();
        *writer += 2;
    }

    let second = *shared.read().unwrap();
    println!("{}", first + second);
}
