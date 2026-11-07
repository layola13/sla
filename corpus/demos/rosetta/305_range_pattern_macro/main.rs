// 305 - Range Patterns
// Rust: match x { 1..=5 => "low", 6..=10 => "mid", 11..=100 => "high", _ => "out" }
fn grade(score: i32) -> i32 {
    match score {
        0..=59 => 0,   // fail
        60..=79 => 1,  // pass
        80..=89 => 2,  // good
        90..=100 => 3, // excellent
        _ => -1,       // invalid
    }
}

fn main() {
    let a = grade(45);
    let b = grade(72);
    let c = grade(85);
    let d = grade(95);
    let e = grade(101);
    println!("{a},{b},{c},{d},{e}");
}
