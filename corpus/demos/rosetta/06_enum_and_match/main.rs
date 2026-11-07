enum Message {
    Quit,
    Move { x: i32, y: i32 },
}

fn process_msg(msg: Message) -> i32 {
    match msg {
        Message::Quit => 0,
        Message::Move { x, y } => x + y,
    }
}

fn main() {
    let msg = Message::Move { x: 10, y: 20 };
    let result = process_msg(msg);
    println!("{result}");
}
