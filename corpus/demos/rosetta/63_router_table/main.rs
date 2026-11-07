use std::collections::HashMap;

fn main() {
    let mut router = HashMap::new();
    router.insert("/home", 1);
    router.insert("/api", 2);
    println!("{}", router["/api"]);
}
