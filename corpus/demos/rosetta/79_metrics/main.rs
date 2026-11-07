fn success_per_thousand(ok: i32, failed: i32) -> i32 {
    ok * 1000 / (ok + failed)
}

fn main() {
    println!("{}", success_per_thousand(95, 5));
}
