pub fn exported_value() -> i32 {
    7
}

fn internal_value() -> i32 {
    5
}

fn main() {
    let total = exported_value() + internal_value();
    println!("{total}");
}
