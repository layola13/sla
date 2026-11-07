#[repr(C)]
struct Row {
    id: i32,
    count: i32,
    pad: i32,
}

extern "C" {
    fn sqlite_insert(row: *const Row) -> i32;
}

fn main() {
    let row = Row { id: 7, count: 1, pad: 0 };
    let code = unsafe { sqlite_insert(&row) };
    println!("{}", code);
}
