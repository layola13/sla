struct Pair {
    first: i32,
    second: i32,
    third: i32,
}

fn blank_identifier_demo() -> i32 {
    let _ = Pair { first: 1, second: 2, third: 3 };
    let [_, second, ..rest] = [10, 20, 30, 40];
    let (w2, _) = (7, 8);
    return second + rest[0] + w2;
}

fn main() {
    let got = blank_identifier_demo();
    assert!(got == 57);
}
