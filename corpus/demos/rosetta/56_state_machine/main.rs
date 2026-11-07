enum State {
    Idle,
    Running,
    Done,
}

fn main() {
    let state = State::Done;
    match state {
        State::Idle => println!("0"),
        State::Running => println!("1"),
        State::Done => println!("2"),
    }
}
