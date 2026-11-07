fn triple_ok(x: i32) -> Result<i32, i32> {
    Ok(x * 3)
}

fn compute() -> Result<i32, i32> {
    let value = triple_ok(7)?;
    Ok(value)
}

fn main() {
    let result = compute().unwrap_or(-1);
    println!("{result}");
}
