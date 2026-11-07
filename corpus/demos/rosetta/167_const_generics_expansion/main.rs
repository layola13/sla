fn array_len<const N: usize>(arr: [i32; N]) -> usize {
    arr.len()
}

fn main() {
    let arr = [1, 2, 3, 4];
    println!("{}", array_len(arr));
}
