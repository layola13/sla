fn main() {
    let value = Some(5).unwrap();
    let err = Err::<i32, i32>(7).unwrap_err();
    println!("{}", value + err);
}
