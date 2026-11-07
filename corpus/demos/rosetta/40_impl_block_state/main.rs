struct Account {
    balance: i32,
}

impl Account {
    fn deposit(&mut self, amount: i32) {
        self.balance += amount;
    }
}

fn main() {
    let mut account = Account { balance: 10 };
    account.deposit(5);
    println!("{}", account.balance);
}
