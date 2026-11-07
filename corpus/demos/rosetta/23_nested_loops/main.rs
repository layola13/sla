fn main() {
    let mut sum = 0;
    for y in 1..=2 {
        for x in 1..=3 {
            sum += x * y;
        }
    }
    println!("{sum}");
}
