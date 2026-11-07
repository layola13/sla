fn custom_dst_pointer_bytes_len_surrogate() -> usize {
    let bytes: &[u8] = b"abc";
    bytes.len()
}

fn main() {
    println!("{}", custom_dst_pointer_bytes_len_surrogate());
}
