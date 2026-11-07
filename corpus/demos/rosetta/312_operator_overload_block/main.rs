fn main() -> i32 {
    let a = Vec2 { x: 4, y: 6 };
    let b = Vec2 { x: 1, y: 3 };
    let c = a + b;
    println!("({}, {})", c.x, c.y);
    if c.x != 5 { return 0; }
    if c.y != 9 { return 0; }
    59
}

#[derive(Clone, Copy)]
struct Vec2 {
    x: i32,
    y: i32,
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;

    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }
}

#[test]
fn operator_overload_block_adds_vectors() {
    let a = Vec2 { x: 4, y: 6 };
    let b = Vec2 { x: 1, y: 3 };
    let c = a + b;
    assert_eq!(c.x, 5);
    assert_eq!(c.y, 9);
}
