use std::marker::PhantomData;

struct Wrapper<T> {
    id: i32,
    _marker: PhantomData<T>,
}

fn main() {
    let w: Wrapper<i64> = Wrapper { id: 7, _marker: PhantomData };
    println!("{}", w.id);
}
