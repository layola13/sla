struct Unit;

fn process(_u: Unit) -> i32 {
    42
}

fn main() {
    let u = Unit;
    let res = process(u);
    println!("{}", res);
}
