type DynError = Box<dyn std::error::Error>;

fn fail() -> Result<&'static str, DynError> {
    Err("anyhow".into())
}

fn main() {
    let result = fail().map(|msg| msg.len());
    println!("{}", result.unwrap_or(0));
}
