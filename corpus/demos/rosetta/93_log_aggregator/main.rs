fn severity_score(info: i32, warn: i32, error: i32, dropped: i32) -> i32 {
    info + warn * 10 + error * 100 - dropped
}

fn main() {
    println!("{}", severity_score(4, 2, 1, 3));
}
