mod math {
    pub fn double(x: i32) -> i32 {
        x * 2
    }
}

fn main() {
    let value = math::double(21);
    println!("{value}");
}
