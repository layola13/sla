fn main() {
    let input = b"Man";
    let b64 = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let q0 = input[0] / 4;
    let q1 = (input[0] % 4) * 16 + input[1] / 16;
    let q2 = (input[1] % 16) * 4 + input[2] / 64;
    let q3 = input[2] % 64;
    let encoded = [
        b64[q0 as usize] as char,
        b64[q1 as usize] as char,
        b64[q2 as usize] as char,
        b64[q3 as usize] as char,
    ];
    let text: String = encoded.iter().collect();
    println!("{}", text);
}
