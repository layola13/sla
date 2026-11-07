#[repr(packed)]
struct Pair {
    a: u8,
    b: u8,
}

fn main() {
    let pair = Pair { a: 1, b: 2 };
    println!("{}", pair.a + pair.b);
}
