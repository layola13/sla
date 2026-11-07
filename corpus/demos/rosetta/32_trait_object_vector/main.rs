trait Score {
    fn score(&self) -> i32;
}

struct AddOne(i32);
struct AddTwo(i32);

impl Score for AddOne {
    fn score(&self) -> i32 { self.0 + 1 }
}

impl Score for AddTwo {
    fn score(&self) -> i32 { self.0 + 2 }
}

fn main() {
    let values: Vec<Box<dyn Score>> = vec![Box::new(AddOne(4)), Box::new(AddTwo(5))];
    let total: i32 = values.iter().map(|v| v.score()).sum();
    println!("{total}");
}
