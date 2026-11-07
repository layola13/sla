struct UnsafeData;
impl !Send for UnsafeData {}

fn main() {
    println!("0");
}
