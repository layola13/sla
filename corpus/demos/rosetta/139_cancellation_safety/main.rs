struct Reservation {
    value: i32,
    committed: bool,
}

impl Reservation {
    fn commit(mut self) -> i32 {
        self.committed = true;
        self.value
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if !self.committed {
            self.value = 0;
        }
    }
}

fn main() {
    let reservation = Reservation {
        value: 4,
        committed: false,
    };
    println!("{}", reservation.commit());
}
