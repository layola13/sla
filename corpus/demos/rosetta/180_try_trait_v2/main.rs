fn add_one(value: Option<i32>) -> Option<i32> {
    let inner = value?;
    Some(inner + 1)
}

fn main() {
    let value = add_one(Some(2)).unwrap();
    println!("{}", value);
}
