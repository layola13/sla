#[derive(Clone, Copy)]
struct Pair {
    left: i32,
    right: i32,
}

fn main() {
    let pair = Pair { left: 1, right: 2 };
    let copy = pair;
    println!("{}", copy.left + copy.right);
}
