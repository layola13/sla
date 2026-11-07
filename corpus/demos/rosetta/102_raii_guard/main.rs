use std::sync::Mutex;

fn inspect(counter: &Mutex<i32>, skip: bool) -> i32 {
    let mut guard = counter.lock().unwrap();
    if skip {
        return *guard;
    }

    *guard += 3;
    *guard
}

fn main() {
    let counter = Mutex::new(0);
    let first = inspect(&counter, true);
    let second = inspect(&counter, false);
    println!("{}", first + second);
}
