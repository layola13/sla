async fn yield_once() -> i32 {
    tokio::task::yield_now().await;
    2
}

#[tokio::main]
async fn main() {
    let yielded = yield_once().await;
    println!("{}", yielded);
}
