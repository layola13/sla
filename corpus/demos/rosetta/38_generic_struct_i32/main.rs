struct Wrapper<T> {
    value: T,
}

fn main() {
    let wrapped = Wrapper { value: 31i32 };
    println!("{}", wrapped.value);
}
