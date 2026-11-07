fn resolver_id(operation: &str, field: &str, nested: bool) -> i32 {
    match (operation, field, nested) {
        ("query", "user", false) => 11,
        ("query", "user", true) => 12,
        ("mutation", "createUser", false) => 21,
        _ => 0,
    }
}

fn main() {
    println!("{}", resolver_id("query", "user", false));
}
