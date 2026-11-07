fn main() {
    let result: i32 = match Some(1) {
        Some(v) => v,
        None => panic!("unreachable"),
    };
    println!("{}", result);
}
