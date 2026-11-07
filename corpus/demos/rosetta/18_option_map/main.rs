fn main() {
    let value = Some(3).map(|x| x + 5).unwrap_or(0);
    println!("{value}");
}
