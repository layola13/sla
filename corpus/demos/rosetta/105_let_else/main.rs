fn main() {
    let value = Some(5);
    let Some(x) = value else {
        println!("0");
        return;
    };

    println!("{x}");
}
