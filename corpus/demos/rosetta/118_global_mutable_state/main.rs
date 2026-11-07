static mut COUNTER: i32 = 0;

fn main() {
    unsafe {
        COUNTER += 2;
        COUNTER += 3;
        println!("{}", COUNTER);
    }
}
