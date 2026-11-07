fn main() {
    let total = ["sa", "asm", "!!"]
        .into_iter()
        .fold(0usize, |acc, item| acc + item.len());
    println!("{total}");
}
