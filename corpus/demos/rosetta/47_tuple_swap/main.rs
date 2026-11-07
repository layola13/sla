fn swap(a: (i32, i32)) -> (i32, i32) {
    (a.1, a.0)
}

fn main() {
    let pair = swap((3, 8));
    println!("{},{}", pair.0, pair.1);
}
