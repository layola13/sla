struct Message {
    actor: i32,
    payload: i32,
}

fn mailbox_score(inbox: [Message; 2]) -> i32 {
    let mut total = 0;
    for message in inbox {
        total += message.actor + message.payload;
    }
    total
}

fn main() {
    let inbox = [
        Message { actor: 1, payload: 2 },
        Message { actor: 2, payload: 3 },
    ];
    println!("{}", mailbox_score(inbox));
}
