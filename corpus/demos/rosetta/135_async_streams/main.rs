use futures::{Stream, StreamExt};

async fn numbers() -> impl Stream<Item = i32> {
    futures::stream::iter([1, 2, 3])
}

#[tokio::main]
async fn main() {
    let mut stream = numbers().await;
    let mut total = 0;

    while let Some(value) = stream.next().await {
        total += value;
    }

    println!("{}", total);
}
