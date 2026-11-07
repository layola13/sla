fn main() {
    let total: i32 = [1, 2, 3, 4, 5].into_iter().filter(|x| x % 2 == 0).sum();
    println!("{total}");
}
