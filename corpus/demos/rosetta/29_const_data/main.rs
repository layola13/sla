const VALUES: [i32; 3] = [1, 2, 3];

fn main() {
    let sum: i32 = VALUES.iter().sum();
    println!("{sum}");
}
