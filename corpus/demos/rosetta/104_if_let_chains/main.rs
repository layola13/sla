fn main() {
    let left = Some(2);
    let middle = Some(3);
    let right = Some(4);

    let result = if let Some(x) = left && let Some(y) = middle && let Some(z) = right {
        x + y + z
    } else {
        0
    };

    println!("{result}");
}
