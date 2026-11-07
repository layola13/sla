fn choose(opt: Option<i32>, fallback: i32) -> i32 {
    opt.unwrap_or(fallback)
}

fn main() {
    let value = choose(Some(9), 1);
    println!("{value}");
}
