type MyIter = impl Iterator<Item = i32>;

fn make_iter() -> MyIter {
    vec![1, 2, 3].into_iter()
}

fn main() {
    let _iter = make_iter();
    println!("0");
}
