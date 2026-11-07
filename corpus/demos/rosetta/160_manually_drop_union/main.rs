use std::mem::ManuallyDrop;

union Slot {
    a: ManuallyDrop<i32>,
    b: ManuallyDrop<i32>,
}

fn main() {
    let slot = Slot { a: ManuallyDrop::new(11) };
    let value = unsafe { ManuallyDrop::into_inner(slot.a) };
    println!("{}", value);
}
