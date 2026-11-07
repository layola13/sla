trait Draw {
    fn draw(&self) -> i32;
}

struct Button {
    id: i32,
}

impl Draw for Button {
    fn draw(&self) -> i32 {
        self.id
    }
}

fn render(item: &dyn Draw) -> i32 {
    item.draw()
}

fn main() {
    let button = Button { id: 77 };
    let result = render(&button);
    println!("{result}");
}
