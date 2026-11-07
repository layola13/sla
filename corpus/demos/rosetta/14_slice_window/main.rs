fn main() {
    let values = [1, 2, 3, 4];
    let window_sum: i32 = values[1..3].iter().sum();
    println!("{window_sum}");
}
