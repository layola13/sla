fn main() {
    let mut value = 5i32;
    let ptr = &mut value as *mut i32;
    let seen = unsafe { std::ptr::read_volatile(ptr) };
    println!("{}", seen);
}
