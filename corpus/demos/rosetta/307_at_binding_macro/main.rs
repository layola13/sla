// 307 - @ Bindings in Patterns
// Rust: match x { n @ 1..=5 => n * 10, n @ 6..=10 => n * 100, _ => 0 }
fn transform(x: i32) -> i32 {
    match x {
        n @ 1..=5 => n * 10,
        n @ 6..=10 => n * 100,
        _ => 0,
    }
}

fn main() {
    let a = transform(3);
    let b = transform(8);
    let c = transform(15);
    println!("{a},{b},{c}");
}
