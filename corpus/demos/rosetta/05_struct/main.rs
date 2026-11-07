struct Point {
    x: i32,
    y: i32,
}

fn create_point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn main() {
    let point = create_point(10, 20);
    println!("({},{})", point.x, point.y);
}
