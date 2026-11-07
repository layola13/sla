fn read_twice(value: &i32) -> i32 {
    *value + *value
}

fn main() {
    let value = 6;
    println!("{}", read_twice(&value));
}
