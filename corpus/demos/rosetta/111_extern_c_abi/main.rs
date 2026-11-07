#[no_mangle]
pub extern "C" fn add_pair(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let result = unsafe { add_pair(11, 12) };
    println!("{}", result);
}
