fn choose_left<'a>(left: &'a i32, _right: &'a i32) -> &'a i32 {
    left
}

fn main() {
    let left = 9;
    let right = 4;
    let chosen = choose_left(&left, &right);
    println!("{chosen}");
}
