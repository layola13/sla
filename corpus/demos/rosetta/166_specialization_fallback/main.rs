#![feature(min_specialization)]

trait Score {
    fn score(&self) -> i32;
}

default impl<T> Score for T {
    default fn score(&self) -> i32 {
        0
    }
}

impl Score for i32 {
    fn score(&self) -> i32 {
        2
    }
}

fn main() {
    let specialized = 1i32.score();
    let fallback = 1u8.score();
    println!("{}", specialized + fallback);
}
