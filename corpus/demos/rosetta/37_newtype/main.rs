struct UserId(u32);

fn main() {
    let user = UserId(42);
    println!("{}", user.0);
}
