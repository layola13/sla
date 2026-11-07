struct OptionValue<T> {
    is_some: bool,
    value: T,
}

fn unwrap_or(opt: OptionValue<i32>, default: i32) -> i32 {
    if opt.is_some { opt.value } else { default }
}

fn main() {
    let opt = OptionValue {
        is_some: true,
        value: 42,
    };
    let result = unwrap_or(opt, 0);
    println!("{result}");
}
