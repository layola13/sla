struct Color(u8, u8, u8);

fn main() {
    let color = Color(2, 4, 8);
    let total = color.0 as i32 + color.1 as i32 + color.2 as i32;
    println!("{total}");
}
