trait MyMarker {}
struct Data { val: i32 }
impl MyMarker for Data {}

fn process<T: MyMarker>(item: &T) -> i32 {
    let _ = item;
    42
}

fn main() {
    let d = Data { val: 42 };
    let res = process(&d);
    println!("{}", res);
}
