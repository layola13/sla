fn bump(value: &mut i32) {
    *value += 1;
}

fn main() {
    let mut value = 9;
    bump(&mut value);
    println!("{value}");
}
