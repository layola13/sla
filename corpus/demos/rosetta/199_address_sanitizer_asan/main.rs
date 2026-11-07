fn main() {
    let mut buf = [0u8; 4];
    buf[0] = 1;
    buf[1] = 2;
    buf[2] = 3;
    buf[3] = 4;
    println!("{}", buf[0] + buf[3]);
}
