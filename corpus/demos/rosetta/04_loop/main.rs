fn fill_zero(arr: &mut [u8; 4]) {
    for i in 0..4 {
        arr[i] = 0;
    }
}

fn main() {
    let mut buffer = [1, 2, 3, 4];
    fill_zero(&mut buffer);
    println!("[{},{},{},{}]", buffer[0], buffer[1], buffer[2], buffer[3]);
}
