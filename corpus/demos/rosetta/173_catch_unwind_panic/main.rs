fn main() {
    let result = std::panic::catch_unwind(|| panic!("stop"));
    let got = if result.is_ok() { 1 } else { 0 };
    println!("{}", got);
}
