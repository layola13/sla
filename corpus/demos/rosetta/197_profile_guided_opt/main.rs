fn hot() -> i32 {
    3
}

fn cold() -> i32 {
    1
}

fn main() {
    let mut total = 0;
    for _ in 0..3 {
        total += hot();
    }
    total += cold();
    println!("{}", total);
}
