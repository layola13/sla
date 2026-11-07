// 309 - Try Blocks
// Rust: let result: Result<i32, _> = try { a()?.add(b()?) };
fn parse_num(s: &str) -> Result<i32, String> {
    s.parse::<i32>().map_err(|e| e.to_string())
}

fn compute() -> Result<i32, String> {
    let result: Result<i32, String> = try {
        let a = parse_num("10")?;
        let b = parse_num("20")?;
        a + b
    };
    result
}

fn main() {
    match compute() {
        Ok(v) => println!("{v}"),
        Err(e) => println!("err: {e}"),
    }
}
