fn frame_valid(kind: u8, len: u8, checksum: u8) -> bool {
    kind == 1 && checksum == kind + len
}

fn main() {
    println!("{}", frame_valid(1, 4, 5));
}
