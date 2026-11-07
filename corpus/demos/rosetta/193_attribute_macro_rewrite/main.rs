fn main() {
    let mut value = 2;
    rewrite_value(&mut value);
    println!("{}", value);
}

#[rewrite]
fn rewrite_value(value: &mut i32) {
    *value += 0;
}
