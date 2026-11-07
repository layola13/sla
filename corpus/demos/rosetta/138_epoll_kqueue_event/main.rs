struct Event {
    token: i32,
    ready: bool,
}

fn main() {
    let events = [
        Event { token: 1, ready: true },
        Event { token: 2, ready: true },
        Event { token: 4, ready: false },
    ];
    let ready_sum: i32 = events
        .iter()
        .filter(|event| event.ready)
        .map(|event| event.token)
        .sum();
    println!("{}", ready_sum);
}
