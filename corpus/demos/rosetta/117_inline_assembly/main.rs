use std::arch::asm;

fn main() {
    let mut value: i32 = 7;
    unsafe {
        asm!("/* native escape */", inout("eax") value);
    }
    println!("{}", value);
}
