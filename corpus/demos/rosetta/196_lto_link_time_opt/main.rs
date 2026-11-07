#[inline(always)]
fn leaf(a: i32, b: i32) -> i32 {
    a + b
}

#[inline(always)]
fn inner(x: i32) -> i32 {
    leaf(x, 1) + leaf(x, 2)
}

#[inline(never)]
fn cold_path(x: i32) -> i32 {
    inner(x) + leaf(3, 4)
}

fn main() {
    let hot = inner(1);
    let cold = cold_path(2);
    println!("{}", hot + cold);
}
