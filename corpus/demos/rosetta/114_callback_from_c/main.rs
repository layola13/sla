extern "C" fn add_one(x: i32) -> i32 {
    x + 1
}

fn apply(cb: extern "C" fn(i32) -> i32, value: i32) -> i32 {
    cb(value)
}

fn main() {
    let result = apply(add_one, 41);
    println!("{}", result);
}
