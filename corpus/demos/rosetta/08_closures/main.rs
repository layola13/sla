fn main() {
    let offset = 5;
    let add_offset = |x: i32| x + offset;
    let result = add_offset(10);
    println!("{result}");
}
