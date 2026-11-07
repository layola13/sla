fn dispatch(command: &str, config_loaded: bool) -> i32 {
    match (command, config_loaded) {
        ("sync", true) => 0,
        ("status", true) => 1,
        _ => 64,
    }
}

fn main() {
    println!("{}", dispatch("sync", true));
}
