fn main() {
    let mut value = 0u32;
    value |= (6u32 & 0x7f) << 0;
    value |= (0u32 & 0x7f) << 7;
    value |= (0u32 & 0x7f) << 14;
    println!("{}", value);
}
