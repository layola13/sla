fn main() {
    let value = Box::new(9);
    std::mem::forget(value);
    println!("9");
}
