fn pair_sum<T>(a: T, b: T) -> (T, T) {
    (a, b)
}

fn main() {
    let pair = pair_sum(11, 31);
    println!("{},{}", pair.0, pair.1);
}
