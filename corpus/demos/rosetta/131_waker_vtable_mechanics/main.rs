use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{RawWaker, RawWakerVTable, Waker};

static WAKE_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe fn clone(data: *const ()) -> RawWaker {
    WAKE_COUNT.fetch_add(1, Ordering::Relaxed);
    RawWaker::new(data, &VTABLE)
}

unsafe fn wake(_data: *const ()) {
    WAKE_COUNT.fetch_add(1, Ordering::Relaxed);
}

unsafe fn wake_by_ref(_data: *const ()) {
    WAKE_COUNT.fetch_add(1, Ordering::Relaxed);
}

unsafe fn drop_waker(_data: *const ()) {}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_waker);

fn main() {
    let raw = RawWaker::new(std::ptr::null(), &VTABLE);
    let waker = unsafe { Waker::from_raw(raw) };
    let cloned = waker.clone();
    cloned.wake_by_ref();
    cloned.wake();
    println!("{}", WAKE_COUNT.load(Ordering::Relaxed));
}
