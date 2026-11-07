struct Counter(i32);

impl Counter {
    fn inc(&mut self) {
        self.0 += 1;
    }
}

fn main() {
    let mut counter = Counter(3);
    counter.inc();
    println!("{}", counter.0);
}
