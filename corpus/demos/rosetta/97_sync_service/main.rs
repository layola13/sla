fn sync_needed(local_version: i32, remote_version: i32, dirty: bool, offline: bool) -> bool {
    if offline {
        false
    } else {
        dirty || local_version < remote_version
    }
}

fn main() {
    println!("{}", sync_needed(7, 9, false, false));
}
