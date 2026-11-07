struct Counter {
    value: i32,
}

impl Counter {
    fn new(seed: i32) -> Self {
        Self { value: seed + 2 }
    }
}

fn main() {
    let counter = Counter::new(40);
    println!("{}", counter.value);
}
