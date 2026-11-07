#[repr(transparent)]
struct Wrap(i32);

fn main() {
    let value = Wrap(7).0;
    println!("{}", value);
}
