enum Maybe<T> {
    Value(T),
    Empty,
}

fn main() {
    let value = Maybe::Value(7i32);
    let result = match value {
        Maybe::Value(v) => v,
        Maybe::Empty => 0,
    };
    println!("{result}");
}
