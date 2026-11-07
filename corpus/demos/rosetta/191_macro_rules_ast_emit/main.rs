macro_rules! emit_sum {
    ($a:expr, $b:expr) => {
        $a + $b
    };
}

fn main() {
    let left = 1;
    let right = 2;
    let total = emit_sum!(left, right);
    println!("{}", total + left + right);
}
