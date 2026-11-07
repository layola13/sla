fn main() {
    let mut i = 0;
    let mut sum = 0;
    loop {
        i += 1;
        if i % 2 == 0 {
            continue;
        }
        if i > 5 {
            break;
        }
        sum += i;
    }
    println!("{sum}");
}
