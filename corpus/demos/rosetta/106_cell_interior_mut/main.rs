use std::cell::Cell;

fn main() {
    let value = Cell::new(10);
    let first = value.get();
    value.set(20);
    let second = value.get();
    println!("{}", first + second);
}
