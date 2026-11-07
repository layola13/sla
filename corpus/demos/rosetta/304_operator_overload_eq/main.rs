#[derive(Debug)]
struct Point {
    x: i32,
    y: i32,
}

impl PartialEq for Point {
    fn eq(&self, other: &Point) -> bool {
        self.x == other.x && self.y == other.y
    }
}

fn main() {
    let a = Point { x: 10, y: 20 };
    let b = Point { x: 10, y: 20 };
    let c = Point { x: 99, y: 0 };

    assert!(a == b);
    assert!(a != c);
    println!("a == b: {}, a != c: {}", a == b, a != c);
}
