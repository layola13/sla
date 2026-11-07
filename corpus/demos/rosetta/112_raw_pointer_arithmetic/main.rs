fn main() {
    let data = [1, 2, 3, 4];
    let third = unsafe { *data.as_ptr().add(2) };
    println!("{}", third);
}
