fn main() {
    let value = Some(5);
    let result = match value {
        Some(x) if x > 3 => x,
        Some(_) => 0,
        None => -1,
    };
    println!("{result}");
}
