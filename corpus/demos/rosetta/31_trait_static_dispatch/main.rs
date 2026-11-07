trait Area {
    fn area(&self) -> i32;
}

struct Square {
    side: i32,
}

impl Area for Square {
    fn area(&self) -> i32 {
        self.side * self.side
    }
}

fn area_of<T: Area>(value: &T) -> i32 {
    value.area()
}

fn main() {
    let square = Square { side: 4 };
    println!("{}", area_of(&square));
}
