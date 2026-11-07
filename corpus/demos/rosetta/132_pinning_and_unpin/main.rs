use std::marker::PhantomPinned;
use std::pin::Pin;

struct PinnedValue {
    value: i32,
    _pin: PhantomPinned,
}

fn main() {
    let pinned = Box::pin(PinnedValue {
        value: 8,
        _pin: PhantomPinned,
    });
    let before = &*pinned as *const PinnedValue;
    let after = Pin::as_ref(&pinned).get_ref() as *const PinnedValue;
    println!("{}", pinned.value + i32::from(before == after));
}
