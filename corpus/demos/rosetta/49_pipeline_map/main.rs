fn main() {
    let values = [1, 2, 3];
    let total: i32 = values.iter().map(|x| x * 2).sum();
    println!("{total}");
}
