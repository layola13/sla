fn fail() -> ! {
    panic!("boom");
}

fn main() {
    // 故意避开调用以防止测试崩溃
    let safe = true;
    if !safe {
        fail();
    }
    println!("0");
}
