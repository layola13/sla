fn max_i32(left: i32, right: i32) -> i32 {
    if left > right { left } else { right }
}

fn critical_path(root: i32, left: i32, right: i32) -> i32 {
    root + max_i32(left, right)
}

fn main() {
    println!("{}", critical_path(4, 7, 3));
}
