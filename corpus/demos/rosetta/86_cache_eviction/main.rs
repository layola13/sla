struct CacheEntry {
    key: i32,
    last_used: i32,
}

fn evict_key(a: CacheEntry, b: CacheEntry, c: CacheEntry) -> i32 {
    let older_ab = if a.last_used < b.last_used { a } else { b };
    if older_ab.last_used < c.last_used { older_ab.key } else { c.key }
}

fn main() {
    println!("{}", evict_key(
        CacheEntry { key: 10, last_used: 4 },
        CacheEntry { key: 20, last_used: 9 },
        CacheEntry { key: 30, last_used: 1 },
    ));
}
