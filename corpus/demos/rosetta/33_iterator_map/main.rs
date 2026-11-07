fn main() {
    let total: i32 = [1, 2, 3].into_iter().map(|x| x * 2).sum();
    println!("{total}");
}
