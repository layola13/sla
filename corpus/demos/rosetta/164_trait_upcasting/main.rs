trait A {
    fn a(&self) -> i32;
}

trait B: A {
    fn b(&self) -> i32;
}

struct Item;

impl A for Item {
    fn a(&self) -> i32 {
        2
    }
}

impl B for Item {
    fn b(&self) -> i32 {
        3
    }
}

fn main() {
    let item = Item;
    println!("{}", item.a() + item.b());
}
