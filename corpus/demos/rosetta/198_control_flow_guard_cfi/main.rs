fn guarded_target(x: i32) -> i32 {
    x + 1
}

fn main() {
    let fp: fn(i32) -> i32 = guarded_target;
    println!("{}", fp(1));
}
