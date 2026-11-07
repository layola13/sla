enum Shape {
    Circle(i32),
    Square(i32),
}

fn area(shape: Shape) -> i32 {
    match shape {
        Shape::Circle(r) => r * r * 3,
        Shape::Square(s) => s * s,
    }
}

fn main() {
    let value = area(Shape::Square(6));
    println!("{value}");
}
