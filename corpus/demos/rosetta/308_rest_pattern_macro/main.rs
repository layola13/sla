// 308 - Rest Patterns (..)
// Rust: [first, second, ..] destructuring, (head, ..) tuple rest
fn sum_first_two(slice: &[i32]) -> i32 {
    match slice {
        [a, b, ..] => a + b,
        _ => 0,
    }
}

fn head(t: &(i32, i32, i32)) -> i32 {
    let (first, ..) = t;
    *first
}

fn main() {
    let arr = [10, 20, 30, 40];
    let s = sum_first_two(&arr);
    let h = head(&(100, 200, 300));
    println!("{s},{h}");
}
