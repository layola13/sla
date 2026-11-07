trait Draw {
    fn draw(&self) -> i32;
}

struct Item {
    value: i32,
}

impl Draw for Item {
    fn draw(&self) -> i32 {
        self.value
    }
}

fn render(item: &dyn Draw) -> i32 {
    item.draw()
}

fn main() {
    let item = Item { value: 4 };
    println!("{}", render(&item));
}
