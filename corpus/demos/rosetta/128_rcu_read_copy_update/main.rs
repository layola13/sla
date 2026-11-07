use std::sync::Arc;

fn main() {
    let old_snapshot = Arc::new(1i32);
    let reader_seen = *old_snapshot;
    let new_snapshot = Arc::new(*old_snapshot + 1);
    let updater_seen = *new_snapshot;
    println!("{}", reader_seen + updater_seen);
}
