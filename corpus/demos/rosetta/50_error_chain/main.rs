fn parse_ok() -> Result<i32, i32> {
    Ok(7)
}

fn main() {
    let value = parse_ok().unwrap();
    println!("{}", value + 5);
}
