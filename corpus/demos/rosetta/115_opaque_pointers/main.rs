struct Opaque;

extern "C" {
    fn opaque_value(ptr: *const Opaque) -> i32;
}

fn main() {
    let ptr = std::ptr::null::<Opaque>();
    let _ = unsafe { opaque_value(ptr) };
    println!("0");
}
