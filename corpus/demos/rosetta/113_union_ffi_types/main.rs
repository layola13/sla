#[repr(C)]
union Payload {
    i: i32,
    b: u8,
}

fn main() {
    let payload = Payload { i: 36 };
    let value = unsafe { payload.i };
    println!("{}", value);
}
