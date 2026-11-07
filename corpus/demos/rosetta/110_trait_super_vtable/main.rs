trait A {
    fn a(&self) -> i32;
}

trait B: A {
    fn b(&self) -> i32;
}

struct Item {
    value: i32,
}

impl A for Item {
    fn a(&self) -> i32 {
        self.value
    }
}

impl B for Item {
    fn b(&self) -> i32 {
        self.value + 1
    }
}

fn main() {
    let item = Item { value: 7 };
    println!("{}", item.a() + item.b());
}
