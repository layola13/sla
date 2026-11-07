struct Vec2 {
    x: i32,
    y: i32,
}

impl Vec2 {
    fn square_len(&self) -> i32 {
        self.x * self.x + self.y * self.y
    }
}

fn main() {
    let v = Vec2 { x: 3, y: 4 };
    println!("{}", v.square_len());
}
