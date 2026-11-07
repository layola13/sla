trait Len {
    fn len(&self) -> i32;
}

impl Len for [i32; 2] {
    fn len(&self) -> i32 {
        2
    }
}

fn main() {
    let arr = [1, 2];
    println!("{}", arr.len());
}
