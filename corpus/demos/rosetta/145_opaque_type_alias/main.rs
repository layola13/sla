trait Maker {
    fn make(&self) -> i32;
}

struct Item;

impl Maker for Item {
    fn make(&self) -> i32 {
        5
    }
}

fn main() {
    let item = Item;
    println!("{}", item.make());
}
