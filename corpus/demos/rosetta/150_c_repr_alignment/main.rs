#[repr(C)]
struct Pair {
    a: u8,
    b: u32,
}

fn main() {
    let pair = Pair { a: 1, b: 2 };
    println!("{}", pair.a as u32 + pair.b);
}
