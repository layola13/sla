async fn task_one() -> i32 {
    1
}

async fn task_two() -> i32 {
    2
}

async fn task_three() -> i32 {
    3
}

#[tokio::main]
async fn main() {
    let mut queue = vec![task_one(), task_two(), task_three()];
    let mut total = 0;

    while let Some(task) = queue.pop() {
        total += task.await;
    }

    println!("{}", total);
}
