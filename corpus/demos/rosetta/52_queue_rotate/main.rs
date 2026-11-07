fn main() {
    let mut queue = std::collections::VecDeque::from([1, 2, 3]);
    queue.rotate_left(1);
    println!("{},{},{}", queue[0], queue[1], queue[2]);
}
