use std::sync::atomic::{AtomicPtr, Ordering};

fn main() {
    let value = Box::new(9i32);
    let shared = AtomicPtr::new(Box::into_raw(value));
    let loaded = shared.load(Ordering::Acquire);
    let hazard = AtomicPtr::new(loaded);
    let still_protected = hazard.load(Ordering::Acquire) == loaded;
    let result = unsafe { *loaded } + i32::from(still_protected);
    println!("{}", result);

    unsafe {
        drop(Box::from_raw(shared.load(Ordering::Acquire)));
    }
}
