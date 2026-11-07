fn main() {
    let raw = Box::into_raw(Box::new(11));
    let boxed = unsafe { Box::from_raw(raw) };
    println!("{}", *boxed);
}
