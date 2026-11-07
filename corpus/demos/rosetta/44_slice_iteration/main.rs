fn sum(values: &[i32]) -> i32 {
    values.iter().copied().sum()
}

fn main() {
    let values = [1, 2, 3, 4];
    println!("{}", sum(&values));
}
