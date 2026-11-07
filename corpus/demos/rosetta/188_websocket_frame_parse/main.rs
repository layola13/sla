fn main() {
    let fin = true;
    let opcode = 1u8;
    let masked = false;
    let payload_len = 3u8;
    println!("{}", if fin && opcode == 1 && !masked && payload_len == 3 { 1 } else { 0 });
}
