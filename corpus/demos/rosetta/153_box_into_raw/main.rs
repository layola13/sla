fn main() {
    let boxed = Box::new(9);
    let raw = Box::into_raw(boxed);
    unsafe {
        println!("{}", *raw);
    }
}
