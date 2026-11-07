fn config_error_message(path: &str) -> String {
    format!("invalid config: {}", path)
}

fn main() {
    let err = config_error_message("oops");
    println!("{}", err);
}
