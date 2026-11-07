use std::collections::HashMap;

fn main() {
    let mut cache = HashMap::new();
    cache.insert("a", 3);
    let hit = cache.get("a").copied().unwrap_or_default();
    println!("{hit}");
}
