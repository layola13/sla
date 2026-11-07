struct Session {
    connected: bool,
    transaction_open: bool,
}

fn can_commit(session: Session) -> bool {
    session.connected && session.transaction_open
}

fn main() {
    let session = Session { connected: true, transaction_open: true };
    println!("{}", can_commit(session));
}
