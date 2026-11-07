#[cfg(target_arch = "x86_64")]
fn main() {
    let lanes = [1i32, 2, 3, 4];
    let sum: i32 = lanes.iter().sum();
    println!("{sum}");
}

#[cfg(not(target_arch = "x86_64"))]
fn main() {
    let lanes = [1i32, 2, 3, 4];
    let sum: i32 = lanes.iter().sum();
    println!("{sum}");
}
