use std::collections::VecDeque;

#[derive(Copy, Clone)]
struct Job {
    id: i32,
    cost: i32,
}

fn main() {
    let mut queue = VecDeque::new();
    queue.push_back(Job { id: 7, cost: 3 });
    queue.push_back(Job { id: 9, cost: 5 });
    let first = queue.pop_front().unwrap();
    println!("{}", first.id + first.cost);
}
