use std::sync::OnceLock;

static VALUE: OnceLock<i32> = OnceLock::new();

fn main() {
    let first = *VALUE.get_or_init(|| 42);
    let second = *VALUE.get_or_init(|| 99);
    println!("{}", first + second);
}
