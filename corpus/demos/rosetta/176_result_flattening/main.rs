fn flatten_result(value: Result<Result<i32, i32>, i32>) -> Result<i32, i32> {
    match value {
        Ok(inner) => inner,
        Err(err) => Err(err),
    }
}

fn main() {
    let nested = Ok(Ok(2));
    let value = flatten_result(nested).unwrap_or(-1);
    println!("{}", value);
}
