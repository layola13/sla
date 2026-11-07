enum Flag {
    A,
    B,
}

fn main() {
    let flag = Flag::B;
    let value = match flag {
        Flag::A => 1,
        Flag::B => 2,
    };
    println!("{value}");
}
