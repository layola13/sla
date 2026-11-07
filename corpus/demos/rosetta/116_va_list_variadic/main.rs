fn sum(nums: &[i32]) -> i32 {
    nums.iter().sum()
}

fn main() {
    let values = [1, 2, 3];
    println!("{}", sum(&values));
}
