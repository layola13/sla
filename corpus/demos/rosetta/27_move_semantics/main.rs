struct Data {
    value: i32,
}

fn consume(data: Data) -> i32 {
    data.value
}

fn main() {
    let data = Data { value: 11 };
    println!("{}", consume(data));
}
