fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

fn main() {
    let result = max(10, 20);
    println!("{result}");
}
